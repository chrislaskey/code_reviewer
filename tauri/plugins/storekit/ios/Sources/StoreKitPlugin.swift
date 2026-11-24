// StoreKit 2 for the app's Rust, see ../../src/lib.rs. A transaction is
// never finished here except when `finish` is asked, after the server
// has recorded it, so nothing paid is lost to a failed request.

import Foundation
import StoreKit
import SwiftRs
import Tauri
import UIKit
import WebKit

struct ProductsArgs: Decodable {
  let productIds: [String]
}

struct PurchaseArgs: Decodable {
  let productId: String
  let appAccountToken: String
}

struct FinishArgs: Decodable {
  let transactionId: String
}

@available(iOS 15.0, *)
class StoreKitPlugin: Plugin {
  /// The products App Store Connect has with the ids, with their price
  /// in the buyer's currency and, for a subscription, its length.
  @objc public func products(_ invoke: Invoke) async throws {
    do {
      let args = try invoke.parseArgs(ProductsArgs.self)
      let products = try await Product.products(for: args.productIds)
      invoke.resolve(["products": products.map(productObject)])
    } catch {
      invoke.reject(error.localizedDescription)
    }
  }

  /// The App Store's sheet for the product, the user's id attached as
  /// `appAccountToken`. Answers `purchased` with the signed transaction,
  /// left unfinished, or `cancelled` or `pending`.
  @objc public func purchase(_ invoke: Invoke) async throws {
    do {
      let args = try invoke.parseArgs(PurchaseArgs.self)
      guard let token = UUID(uuidString: args.appAccountToken) else {
        invoke.reject("The user id is not a UUID")
        return
      }
      guard let product = try await Product.products(for: [args.productId]).first else {
        invoke.reject("The App Store has no product \(args.productId)")
        return
      }
      switch try await product.purchase(options: [.appAccountToken(token)]) {
      case .success(let verification):
        var answer = transactionObject(verification)
        answer["outcome"] = "purchased"
        invoke.resolve(answer)
      case .userCancelled:
        invoke.resolve(["outcome": "cancelled"])
      case .pending:
        invoke.resolve(["outcome": "pending"])
      @unknown default:
        invoke.reject("The App Store answered something unknown")
      }
    } catch {
      invoke.reject(error.localizedDescription)
    }
  }

  /// Every transaction not finished yet: paid, and not yet recorded by
  /// the server as far as this device knows.
  @objc public func unfinished(_ invoke: Invoke) async throws {
    var transactions: [[String: Any]] = []
    for await verification in Transaction.unfinished {
      transactions.append(transactionObject(verification))
    }
    invoke.resolve(["transactions": transactions])
  }

  /// Restore purchases: syncs with the App Store, which may ask the
  /// person to sign in, then every transaction that still entitles
  /// this Apple ID to something.
  @objc public func restore(_ invoke: Invoke) async throws {
    do {
      try await AppStore.sync()
      var transactions: [[String: Any]] = []
      for await verification in Transaction.currentEntitlements {
        transactions.append(transactionObject(verification))
      }
      invoke.resolve(["transactions": transactions])
    } catch {
      invoke.reject(error.localizedDescription)
    }
  }

  /// Finishes the transaction, once the server has it.
  @objc public func finish(_ invoke: Invoke) async throws {
    do {
      let args = try invoke.parseArgs(FinishArgs.self)
      for await verification in Transaction.unfinished {
        let transaction = verification.unsafePayloadValue
        if String(transaction.id) == args.transactionId {
          await transaction.finish()
        }
      }
      invoke.resolve()
    } catch {
      invoke.reject(error.localizedDescription)
    }
  }

  /// The App Store's own page of this Apple ID's subscriptions, where a
  /// plan is cancelled or resumed.
  @objc public func manageSubscriptionsOpen(_ invoke: Invoke) async throws {
    let scene = await MainActor.run {
      UIApplication.shared.connectedScenes.first { $0 is UIWindowScene } as? UIWindowScene
    }
    guard let scene else {
      invoke.reject("No window to show the App Store's page in")
      return
    }
    do {
      try await AppStore.showManageSubscriptions(in: scene)
      invoke.resolve()
    } catch {
      invoke.reject(error.localizedDescription)
    }
  }

  private func productObject(_ product: Product) -> [String: Any] {
    var object: [String: Any] = [
      "id": product.id,
      "displayName": product.displayName,
      "displayPrice": product.displayPrice,
    ]
    if let period = product.subscription?.subscriptionPeriod {
      object["period"] = periodName(period)
    }
    return object
  }

  private func periodName(_ period: Product.SubscriptionPeriod) -> String {
    switch period.unit {
    case .day: return period.value == 7 ? "week" : "\(period.value) days"
    case .week: return period.value == 1 ? "week" : "\(period.value) weeks"
    case .month: return period.value == 1 ? "month" : "\(period.value) months"
    case .year: return period.value == 1 ? "year" : "\(period.value) years"
    @unknown default: return "period"
    }
  }

  private func transactionObject(_ verification: VerificationResult<Transaction>) -> [String: Any] {
    let transaction = verification.unsafePayloadValue
    return [
      "transactionId": String(transaction.id),
      "productId": transaction.productID,
      "signedTransaction": verification.jwsRepresentation,
    ]
  }
}

@_cdecl("init_plugin_storekit")
public func initPlugin() -> Plugin {
  return StoreKitPlugin()
}
