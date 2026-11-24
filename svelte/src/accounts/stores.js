// Where a plan or product was bought, as the pages name it. The pages
// read `store` from what the server and the commands answer, never the
// platform, see docs/design/purchasing-phones.md.
import { t } from "ttag";

/** @typedef {"stripe" | "app_store" | "play_store"} Store */

/**
 * `the App Store`
 * @param {Store} store
 */
export function storeName(store) {
  if (store === "app_store") return t`the App Store`;
  if (store === "play_store") return t`Google Play`;
  return t`the web`;
}

/**
 * `in the App Store on your iPhone`, where a plan from another store is managed.
 * @param {Store} store
 */
export function storeManagedWhere(store) {
  if (store === "app_store") return t`in the App Store on your iPhone`;
  if (store === "play_store") return t`in Google Play on your Android device`;
  return t`on the web`;
}

/**
 * A price as the phone's store sells it, `$9.99 / month`, else as the server has it.
 * @param {{store_price?: string | null, store_period?: string | null}} price
 * @param {(price: any) => string} serverFormat
 */
export function storePriceFormat(price, serverFormat) {
  if (!price.store_price) return serverFormat(price);
  return price.store_period ? `${price.store_price} / ${price.store_period}` : price.store_price;
}
