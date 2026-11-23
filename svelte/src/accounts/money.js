// A price as the pages show it. The server keeps an amount in the
// currency's smallest unit, cents for usd; Intl knows how many digits a
// currency has, none for jpy.
import { t } from "ttag";
import { translationsFormatLocale } from "../lib/translations.js";

/**
 * @param {number} amount in the currency's smallest unit
 * @param {string} currency lowercase, `usd`
 */
export function moneyFormat(amount, currency) {
  const format = new Intl.NumberFormat(translationsFormatLocale(), { style: "currency", currency: currency.toUpperCase() });
  const digits = format.resolvedOptions().maximumFractionDigits ?? 2;
  return format.format(amount / 10 ** digits);
}

/**
 * `$9.00 / month`, or `$49.00` for a one-time price.
 * @param {{amount: number, currency: string, interval: "month" | "year" | null}} price
 */
export function priceFormat(price) {
  const money = moneyFormat(price.amount, price.currency);
  if (price.interval === "month") return t`${money} / month`;
  if (price.interval === "year") return t`${money} / year`;
  return money;
}
