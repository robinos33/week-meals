/**
 * Rapprochement d'un nom d'ingrédient saisi avec ce que l'on connaît déjà :
 * les articles de la liste courante et le dictionnaire de l'API.
 *
 * Pendant côté saisie du rapprochement fait par l'API à la génération et à
 * l'ajout (`shopping-list/domain/matching.rs`) : mêmes règles de normalisation
 * — casse, accents, ligatures, ponctuation, mots vides, pluriels — pour que
 * la suggestion affichée soit bien la ligne sur laquelle l'API cumulera.
 *
 * La différence est d'usage : ici on classe des candidats pour une frappe en
 * cours (« cour » doit proposer « courgette »), là-bas on décide d'une
 * égalité. D'où le préfixe et la sous-chaîne en plus, et un seuil de
 * ressemblance plus généreux — une suggestion à côté de la plaque se corrige
 * d'un caractère de plus, elle ne casse rien.
 */

/** Mots vides du vocabulaire culinaire (sans valeur de produit). */
const STOP_WORDS = new Set([
  "de",
  "du",
  "des",
  "d",
  "la",
  "le",
  "les",
  "l",
  "au",
  "aux",
  "a",
  "en",
  "et",
  "un",
  "une",
]);

/**
 * Qualificatifs qui ne changent **pas ce qu'on met dans le caddie** : calibre,
 * maturité, mode de culture, découpe faite à la maison. Retirés seulement pour
 * la « clé produit ».
 *
 * Miroir exact de la table du serveur (`shopping-list/domain/matching.rs`) —
 * les couleurs en sont volontairement absentes : « poivron rouge » n'est pas
 * « poivron jaune », pas plus que « lait de coco » n'est du lait. Toute
 * addition ici doit passer le test « je repars du magasin avec le même
 * produit sans ce mot ».
 */
const QUALIFIERS = new Set([
  "bio",
  "frais",
  "fraiche",
  "gros",
  "grosse",
  "petit",
  "petite",
  "moyen",
  "moyenne",
  "grand",
  "grande",
  "fin",
  "fine",
  "long",
  "longue",
  "rond",
  "ronde",
  "emince",
  "emincee",
  "coupe",
  "coupee",
  "cru",
  "crue",
  "cuit",
  "cuite",
  "mur",
  "mure",
  "bien",
  "nature",
  "extra",
]);

/** Minuscule sans accent ni ligature (« Œufs » → « oeufs »). */
function fold(value: string): string {
  return value
    .toLowerCase()
    .replace(/œ/g, "oe")
    .replace(/æ/g, "ae")
    .normalize("NFD")
    .replace(/\p{Diacritic}/gu, "");
}

/**
 * Singulier « cohérent » plutôt que savant : seule compte l'égalité des deux
 * côtés de la comparaison (« ananas » devient « anana », et alors ?).
 */
function singular(token: string): string {
  if (token.length > 4 && token.endsWith("aux")) return `${token.slice(0, -3)}al`;
  if (token.length > 3 && (token.endsWith("s") || token.endsWith("x"))) return token.slice(0, -1);
  return token;
}

/** Mots normalisés d'un nom : dépliés, sans ponctuation ni mots vides, au singulier. */
function tokens(name: string): string[] {
  return fold(name)
    .split(/[^a-z0-9]+/)
    .filter((word) => word && !STOP_WORDS.has(word))
    .map(singular);
}

/** Clé de rapprochement d'un nom (« Œufs de poule » → « oeuf poule »). */
export function canonicalKey(name: string): string {
  return tokens(name).join(" ");
}

/** Clé produit, qualificatifs retirés (« grosses courgettes » → « courgette »). */
export function coreKey(name: string): string {
  return tokens(name)
    .filter((token) => !QUALIFIERS.has(token))
    .join(" ");
}

/** Distance d'édition, en mémoire O(min(n, m)). */
function levenshtein(left: string, right: string): number {
  if (!left.length) return right.length;
  if (!right.length) return left.length;

  let previous = Array.from({ length: right.length + 1 }, (_, i) => i);
  for (let i = 0; i < left.length; i += 1) {
    const current = [i + 1];
    for (let j = 0; j < right.length; j += 1) {
      current[j + 1] = Math.min(
        previous[j] + (left[i] === right[j] ? 0 : 1),
        previous[j + 1] + 1, // suppression
        current[j] + 1, // insertion
      );
    }
    previous = current;
  }
  return previous[right.length];
}

/** Proximité de deux clés, de 0 (rien en commun) à 1 (identiques). */
export function similarity(left: string, right: string): number {
  if (left === right) return 1;
  const longest = Math.max(left.length, right.length);
  return longest === 0 ? 1 : 1 - levenshtein(left, right) / longest;
}

/** En deçà, la ressemblance n'est plus qu'un hasard : on ne suggère pas. */
const FUZZY_THRESHOLD = 0.7;
/** Longueur minimale d'une saisie pour tenter la ressemblance approximative. */
const MIN_FUZZY_LENGTH = 4;

/**
 * Score de pertinence d'un candidat pour la saisie en cours, de 0 à 1, ou
 * `null` s'il n'a rien à faire dans la liste des suggestions.
 *
 * Par ordre décroissant : nom identique, préfixe (« cour » → « courgette »),
 * même produit aux qualificatifs près, mot commençant par la saisie
 * (« pomme » → « pomme de terre »), sous-chaîne, puis ressemblance
 * approximative (« echalotte » → « échalote »). `aliases` élargit la
 * reconnaissance sans changer le libellé affiché.
 */
export function matchScore(query: string, name: string, aliases: string[] = []): number | null {
  const wanted = canonicalKey(query);
  if (!wanted) return null;

  let best: number | null = null;
  const keep = (score: number) => {
    if (best === null || score > best) best = score;
  };

  for (const candidate of [name, ...aliases]) {
    const key = canonicalKey(candidate);
    if (!key) continue;

    if (key === wanted) {
      keep(1);
      continue;
    }
    if (key.startsWith(wanted)) {
      // Plus le reste est court, plus la suggestion est proche du but.
      keep(0.9 - Math.min(key.length - wanted.length, 20) / 200);
      continue;
    }
    if (coreKey(candidate) === coreKey(query)) {
      keep(0.85);
      continue;
    }
    if (key.split(" ").some((word) => word.startsWith(wanted))) {
      keep(0.75);
      continue;
    }
    if (key.includes(wanted)) {
      keep(0.7);
      continue;
    }
    if (wanted.length >= MIN_FUZZY_LENGTH && key.length >= MIN_FUZZY_LENGTH) {
      const score = similarity(wanted, key);
      if (score >= FUZZY_THRESHOLD) keep(score * 0.65);
    }
  }

  return best;
}

/** Un candidat à la suggestion, quelle que soit sa provenance. */
export interface Matchable {
  /** Libellé affiché et inséré dans le champ. */
  name: string;
  /** Synonymes reconnus à la frappe (jamais affichés). */
  aliases?: string[];
}

/**
 * Classe des candidats pour la saisie en cours, du plus pertinent au moins
 * pertinent, en écartant ceux qui ne matchent pas.
 *
 * `boost` permet de faire remonter une famille de candidats à pertinence
 * comparable — les articles **déjà dans la liste** passent devant le
 * dictionnaire, puisque c'est sur eux que la quantité se cumulera.
 */
export function rankMatches<T extends Matchable>(
  query: string,
  candidates: readonly T[],
  boost: (candidate: T) => number = () => 0,
): T[] {
  return candidates
    .map((candidate) => ({
      candidate,
      score: matchScore(query, candidate.name, candidate.aliases),
    }))
    .filter((scored): scored is { candidate: T; score: number } => scored.score !== null)
    .map((scored) => ({ ...scored, score: scored.score + boost(scored.candidate) }))
    .sort((a, b) => b.score - a.score || a.candidate.name.localeCompare(b.candidate.name))
    .map((scored) => scored.candidate);
}
