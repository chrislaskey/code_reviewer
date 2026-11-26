// ASWebAuthenticationSession for the app's Rust, see ../../src/lib.rs.
// The session is kept until it ends, since the sheet goes with it, and a
// new one cancels the one before.

import AuthenticationServices
import Foundation
import SwiftRs
import Tauri
import UIKit
import WebKit

struct OpenArgs: Decodable {
  let url: String
  let callbackScheme: String
}

class WebAuthSessionPlugin: Plugin, ASWebAuthenticationPresentationContextProviding {
  private var session: ASWebAuthenticationSession?
  private weak var appWebView: WKWebView?

  /// Keeps the app webview for layout after the sign-in sheet closes.
  public override func load(webview: WKWebView) {
    appWebView = webview
  }

  /// The sheet at the URL, until a page in it goes to the scheme:
  /// answers `returned` with that URL, or `cancelled` when the person
  /// closed the sheet. Private, sharing no cookies with Safari, so iOS
  /// asks no permission to open it.
  @objc public func open(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(OpenArgs.self)
    guard let url = URL(string: args.url) else {
      invoke.reject("Not a URL: \(args.url)")
      return
    }
    DispatchQueue.main.async {
      self.session?.cancel()
      let session = ASWebAuthenticationSession(url: url, callbackURLScheme: args.callbackScheme) {
        returnedUrl, error in
        DispatchQueue.main.async {
          self.session = nil
          if let returnedUrl {
            self.restoreAppLayout()
            invoke.resolve(["outcome": "returned", "url": returnedUrl.absoluteString])
          } else if let error = error as? ASWebAuthenticationSessionError, error.code == .canceledLogin {
            invoke.resolve(["outcome": "cancelled"])
          } else {
            invoke.reject(error?.localizedDescription ?? "The sign-in sheet ended with nothing")
          }
        }
      }
      session.presentationContextProvider = self
      session.prefersEphemeralWebBrowserSession = true
      self.session = session
      if !session.start() {
        self.session = nil
        invoke.reject("The sign-in sheet could not open")
      }
    }
  }

  // Sheet dismissal can leave the app viewport at zero size.
  private func restoreAppLayout() {
    guard let webView = appWebView, let parent = webView.superview else { return }
    webView.window?.setNeedsLayout()
    webView.window?.layoutIfNeeded()
    parent.setNeedsLayout()
    parent.layoutIfNeeded()
    if parent.bounds.width > 0 && parent.bounds.height > 0 {
      webView.frame = parent.bounds
    }
    webView.setNeedsLayout()
    webView.layoutIfNeeded()
  }

  func presentationAnchor(for session: ASWebAuthenticationSession) -> ASPresentationAnchor {
    let windows = UIApplication.shared.connectedScenes
      .compactMap { $0 as? UIWindowScene }
      .flatMap { $0.windows }
    return windows.first { $0.isKeyWindow } ?? windows.first ?? ASPresentationAnchor()
  }
}

@_cdecl("init_plugin_web_auth_session")
func initPlugin() -> Plugin {
  return WebAuthSessionPlugin()
}
