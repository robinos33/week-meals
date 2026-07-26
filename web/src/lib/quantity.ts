/**
 * Formatage et lecture des quantités en fractions.
 *
 * Le stockage reste numérique (l'API manipule des `f64`) : le moteur de liste
 * de courses somme et convertit sur des nombres. Les fractions ne vivent qu'aux
 * bords — affichage (« ½ », « 1½ ») et saisie (« 1/2 », « 1 1/2 », « 0,5 ») —
 * notamment quand une recette « pour 4 » est ramenée à 2 personnes (½ œuf) ou
 * qu'on l'affiche pour un nombre de convives impair.
 */

/** Fractions usuelles → glyphe, de la plus « fine » à la plus grossière. */
const FRACTIONS: ReadonlyArray<{ value: number; glyph: string }> = [
  { value: 1 / 8, glyph: "⅛" },
  { value: 1 / 4, glyph: "¼" },
  { value: 1 / 3, glyph: "⅓" },
  { value: 3 / 8, glyph: "⅜" },
  { value: 1 / 2, glyph: "½" },
  { value: 5 / 8, glyph: "⅝" },
  { value: 2 / 3, glyph: "⅔" },
  { value: 3 / 4, glyph: "¾" },
  { value: 7 / 8, glyph: "⅞" },
];

/** Tolérance d'accrochage à une fraction usuelle (⅓ = 0,333… n'est pas exact). */
const TOLERANCE = 0.02;

/**
 * Formate un montant : entier tel quel, sinon en fraction usuelle (« ¾ »,
 * « 1½ ») quand la partie décimale s'en approche, sinon en décimal court
 * (au plus 2 décimales, sans zéro superflu).
 */
export function formatAmount(amount: number): string {
  if (!Number.isFinite(amount)) return "0";
  if (Number.isInteger(amount)) return String(amount);

  const whole = Math.floor(amount);
  const frac = amount - whole;

  const match = FRACTIONS.find((f) => Math.abs(frac - f.value) <= TOLERANCE);
  if (match) {
    return whole > 0 ? `${whole}${match.glyph}` : match.glyph;
  }

  // Pas de fraction usuelle : décimal court (« 1.25 », « 0.4 »).
  return String(Number(amount.toFixed(2)));
}

/**
 * Formate un montant en décimal court : entier tel quel, sinon au plus 2
 * décimales sans zéro superflu (« 250 », « 1.5 », « 0.4 »). Pour les masses et
 * volumes, où « 1.5 kg » se lit mieux que « 1½ kg » — les fractions sont
 * réservées aux pièces (« ½ œuf »).
 */
export function formatDecimal(amount: number): string {
  if (!Number.isFinite(amount)) return "0";
  return Number.isInteger(amount) ? String(amount) : String(Number(amount.toFixed(2)));
}

/**
 * Lit un montant saisi en tolérant les fractions et la virgule décimale :
 * « 1/2 » → 0.5, « 1 1/2 » → 1.5, « 0,75 » → 0.75, « 2 » → 2. Renvoie `null`
 * si l'entrée n'est pas un nombre strictement positif exploitable.
 */
export function parseAmount(input: string): number | null {
  const text = input.trim().replace(",", ".");
  if (!text) return null;

  // « a b/c » (nombre mixte), « b/c » (fraction), ou décimal simple.
  const mixed = /^(\d+)\s+(\d+)\s*\/\s*(\d+)$/.exec(text);
  const fraction = /^(\d+)\s*\/\s*(\d+)$/.exec(text);

  let value: number;
  if (mixed) {
    const [, whole, num, den] = mixed;
    if (Number(den) === 0) return null;
    value = Number(whole) + Number(num) / Number(den);
  } else if (fraction) {
    const [, num, den] = fraction;
    if (Number(den) === 0) return null;
    value = Number(num) / Number(den);
  } else {
    value = Number(text);
  }

  return Number.isFinite(value) && value > 0 ? value : null;
}
