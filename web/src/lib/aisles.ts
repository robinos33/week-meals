/**
 * Regroupement de la liste de courses **par rayon**, dans l'ordre de visite
 * d'un magasin.
 *
 * C'est la moitié visible du paramétrage des magasins : le magasin dit dans
 * quel ordre on traverse les rayons, cette fonction découpe la liste en
 * sections dans cet ordre. Rien de plus — le tri ne modifie jamais la liste
 * côté serveur, il ne fait que l'afficher autrement.
 *
 * Deux garde-fous, parce qu'un article ne doit **jamais** disparaître d'une
 * liste de courses :
 *
 * - un article sans rayon (produit absent du dictionnaire, saisi à la main)
 *   comme un article rangé dans un rayon que le magasin ignore atterrissent
 *   dans une section « Autres », en fin de parcours ;
 * - un rayon vide ne produit pas de section.
 */

/** Emoji d'un rayon, pour donner un repère visuel aux en-têtes de section. */
const AISLE_EMOJI: Record<string, string> = {
  fruits: "🍎",
  legumes: "🥕",
  boulangerie: "🥖",
  boucherie: "🥩",
  charcuterie: "🥓",
  poissonnerie: "🐟",
  cremerie: "🧀",
  surgeles: "🧊",
  "epicerie-salee": "🍝",
  "epicerie-sucree": "🍫",
  condiments: "🧂",
  boissons: "🥤",
  hygiene: "🧴",
  entretien: "🧽",
  maison: "🔌",
  bebe: "🍼",
  animaux: "🐾",
};

/** Libellé de la section fourre-tout, toujours en fin de parcours. */
export const OTHER_AISLE_LABEL = "Autres";

/** Emoji d'un rayon (`•` pour un rayon inconnu ou la section « Autres »). */
export function aisleEmoji(slug: string | null): string {
  return (slug && AISLE_EMOJI[slug]) || "•";
}

/** Une section de la liste : un rayon et ses articles. */
export interface AisleGroup<T> {
  /** Slug du rayon, `null` pour la section « Autres ». */
  slug: string | null;
  /** Libellé affiché de la section. */
  label: string;
  /** Articles du rayon, dans leur ordre d'origine. */
  items: T[];
}

/**
 * Découpe `items` en sections de rayon, dans l'ordre `order`.
 *
 * `labels` donne le libellé de chaque slug (catalogue servi par l'API) ; un
 * slug sans libellé connu s'affiche tel quel plutôt que de disparaître.
 */
export function groupByAisle<T extends { category: string | null }>(
  items: T[],
  order: string[],
  labels: Record<string, string>,
): AisleGroup<T>[] {
  const sections = new Map<string, T[]>(order.map((slug) => [slug, []]));
  const others: T[] = [];

  for (const item of items) {
    const bucket = item.category ? sections.get(item.category) : undefined;
    if (bucket) bucket.push(item);
    else others.push(item);
  }

  const groups: AisleGroup<T>[] = order
    .filter((slug) => (sections.get(slug)?.length ?? 0) > 0)
    .map((slug) => ({
      slug,
      label: labels[slug] ?? slug,
      items: sections.get(slug) ?? [],
    }));

  if (others.length > 0) {
    groups.push({ slug: null, label: OTHER_AISLE_LABEL, items: others });
  }
  return groups;
}
