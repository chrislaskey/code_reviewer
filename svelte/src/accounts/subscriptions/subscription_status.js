// A subscription's status as the pages say it.

/** @type {Record<string, string>} */
const SAID = {
  active: "Active",
  trialing: "In its free days",
  past_due: "Payment failed, Stripe is retrying",
  unpaid: "Payment failed",
  incomplete: "Waiting for the first payment",
  incomplete_expired: "Never paid",
  paused: "Paused",
  canceled: "Ended",
};

/** @param {string} status */
export function subscriptionStatusSay(status) {
  return SAID[status] ?? status;
}
