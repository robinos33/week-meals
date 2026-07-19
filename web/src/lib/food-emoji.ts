/**
 * Devine l'emoji d'un aliment à partir de son nom (français), pour l'afficher
 * automatiquement dans la liste de courses sans que l'utilisateur le saisisse.
 *
 * On se limite à des emojis largement supportés sur les différents OS (pas de
 * pictos trop récents comme les pois 🫛 ou le gingembre 🫚) pour éviter les
 * carrés « tofu » sur les appareils un peu anciens.
 *
 * Le rapprochement se fait par **mots entiers** : on découpe le nom en jetons
 * normalisés (minuscules, sans accent) et une règle matche si ses mots
 * apparaissent tels quels (le pluriel régulier en `-s` est toléré). Ainsi
 * « poire » ne matche pas « poireau », et « pâte brisée » ne devient pas 🍝.
 */

/** Minuscule sans accent, ponctuation réduite à des espaces. */
function normalize(value: string): string {
  return value
    .toLowerCase()
    .replace(/œ/g, "oe")
    .replace(/æ/g, "ae")
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/** Un jeton correspond à un mot de règle, au singulier comme au pluriel en -s. */
function tokenMatches(token: string, word: string): boolean {
  return token === word || token === `${word}s`;
}

/**
 * Table nom → emoji. **L'ordre compte** : les libellés composés et les cas
 * ambigus passent avant leurs sous-mots (« pomme de terre » avant « pomme »,
 * « coco » avant « lait »).
 */
const RULES: ReadonlyArray<readonly [string, string]> = [
  // Composés & désambiguïsations (à garder en tête de liste).
  ["pomme de terre", "🥔"],
  ["patate douce", "🍠"],
  ["noix de coco", "🥥"],
  ["coco", "🥥"],
  ["citron vert", "🍋"],

  // Fruits.
  ["pomme", "🍎"],
  ["poire", "🍐"],
  ["banane", "🍌"],
  ["orange", "🍊"],
  ["clementine", "🍊"],
  ["mandarine", "🍊"],
  ["citron", "🍋"],
  ["fraise", "🍓"],
  ["framboise", "🍓"],
  ["myrtille", "🫐"],
  ["raisin", "🍇"],
  ["pasteque", "🍉"],
  ["melon", "🍈"],
  ["cerise", "🍒"],
  ["peche", "🍑"],
  ["abricot", "🍑"],
  ["ananas", "🍍"],
  ["mangue", "🥭"],
  ["kiwi", "🥝"],
  ["tomate", "🍅"],
  ["avocat", "🥑"],
  ["olive", "🫒"],

  // Légumes.
  ["patate", "🥔"],
  ["carotte", "🥕"],
  ["mais", "🌽"],
  ["piment", "🌶️"],
  ["poivron", "🫑"],
  ["concombre", "🥒"],
  ["courgette", "🥒"],
  ["salade", "🥬"],
  ["laitue", "🥬"],
  ["epinard", "🥬"],
  ["brocoli", "🥦"],
  ["poireau", "🥬"],
  ["ail", "🧄"],
  ["oignon", "🧅"],
  ["echalote", "🧅"],
  ["champignon", "🍄"],
  ["aubergine", "🍆"],
  ["haricot", "🫘"],
  ["lentille", "🫘"],
  ["pois chiche", "🫘"],

  // Féculents & pain.
  ["pain", "🍞"],
  ["baguette", "🥖"],
  ["croissant", "🥐"],
  ["brioche", "🥐"],
  ["riz", "🍚"],
  ["pates", "🍝"],
  ["spaghetti", "🍝"],
  ["farine", "🌾"],

  // Protéines.
  ["oeuf", "🥚"],
  ["poulet", "🍗"],
  ["dinde", "🍗"],
  ["boeuf", "🥩"],
  ["steak", "🥩"],
  ["viande", "🥩"],
  ["porc", "🥓"],
  ["lard", "🥓"],
  ["bacon", "🥓"],
  ["jambon", "🍖"],
  ["saucisse", "🌭"],
  ["poisson", "🐟"],
  ["saumon", "🐟"],
  ["thon", "🐟"],
  ["crevette", "🦐"],
  ["crabe", "🦀"],
  ["homard", "🦞"],
  ["calamar", "🦑"],
  ["huitre", "🦪"],

  // Produits laitiers.
  ["lait", "🥛"],
  ["fromage", "🧀"],
  ["beurre", "🧈"],

  // Sucré & divers.
  ["miel", "🍯"],
  ["chocolat", "🍫"],
  ["biscuit", "🍪"],
  ["cookie", "🍪"],
  ["gateau", "🍰"],
  ["glace", "🍦"],
  ["bonbon", "🍬"],
  ["soupe", "🍲"],
  ["pizza", "🍕"],
  ["burger", "🍔"],
  ["hamburger", "🍔"],
  ["frite", "🍟"],
  ["sandwich", "🥪"],
  ["sushi", "🍣"],
  ["basilic", "🌿"],
  ["herbe", "🌿"],
  ["cacahuete", "🥜"],
  ["noix", "🌰"],

  // Boissons.
  ["cafe", "☕"],
  ["the", "🍵"],
  ["vin", "🍷"],
  ["biere", "🍺"],
  ["jus", "🧃"],
  ["soda", "🥤"],
  ["eau", "💧"],
];

/** Règles pré-découpées en mots normalisés (calculé une fois au chargement). */
const COMPILED = RULES.map(([label, emoji]) => ({
  words: normalize(label).split(" "),
  emoji,
}));

/** Vrai si les `words` d'une règle apparaissent consécutivement dans `tokens`. */
function matches(tokens: string[], words: string[]): boolean {
  for (let i = 0; i + words.length <= tokens.length; i += 1) {
    if (words.every((word, j) => tokenMatches(tokens[i + j], word))) return true;
  }
  return false;
}

/** Emoji correspondant au nom d'un aliment, ou `null` si aucun ne colle. */
export function foodEmoji(name: string): string | null {
  const tokens = normalize(name).split(" ").filter(Boolean);
  if (tokens.length === 0) return null;
  for (const rule of COMPILED) {
    if (matches(tokens, rule.words)) return rule.emoji;
  }
  return null;
}
