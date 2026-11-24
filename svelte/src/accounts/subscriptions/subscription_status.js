// A subscription's status as the pages say it.
import { t } from "ttag";

/** @param {string} status */
export function subscriptionStatusSay(status) {
  switch (status) {
    case "active":
      return t`Active`;
    case "trialing":
      return t`In its free days`;
    case "past_due":
      return t`Payment failed, Stripe is retrying`;
    case "unpaid":
      return t`Payment failed`;
    case "incomplete":
      return t`Waiting for the first payment`;
    case "incomplete_expired":
      return t`Never paid`;
    case "paused":
      return t`Paused`;
    case "canceled":
      return t`Ended`;
    default:
      return status;
  }
}
