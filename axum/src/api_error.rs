//! An error as the API answers it: a status and `{"error": "..."}`, from
//! the error of whichever core module the handler called; a form's
//! fields that were wrong also as `"errors": {"field": ["message"]}`;
//! a token of a deleted account as 410 with `"code": "account_deleted"`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use app_core::accounts::AccountError;
use app_core::accounts::app_store_notifications::AppStoreNotificationError;
use app_core::accounts::checkouts::CheckoutError;
use app_core::accounts::deletions::DeletionError;
use app_core::accounts::plans::PlanError;
use app_core::accounts::products::ProductError;
use app_core::accounts::purchases::PurchaseError;
use app_core::accounts::store_transactions::StoreTransactionError;
use app_core::accounts::stripe_events::StripeEventError;
use app_core::accounts::subscriptions::SubscriptionError;
use app_core::accounts::users::AccountUserError;
use app_core::app::avatars::AvatarError;
use app_core::app::communities::CommunityError;
use app_core::app::current_community::CurrentCommunityError;
use app_core::app::events::RecordError;
use app_core::app::files::FileError;
use app_core::app::files_communities::FileCommunityError;
use app_core::app::memberships::MembershipError;
use app_core::app::posts::PostError;
use app_core::app::posts_communities::PostCommunityError;
use app_core::app::sync::SyncError;
use app_core::app::users::UserError;
use app_core::credentials::CredentialError;
use app_core::helpers::field_errors::FieldErrors;
use app_core::services::app_store::AppStoreError;
use app_core::services::stripe::StripeError;

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    message: String,
    field_errors: Option<FieldErrors>,
    /// A word the apps act on, beside the message for the person.
    code: Option<&'static str>,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> ApiError {
        ApiError {
            status,
            message: message.into(),
            field_errors: None,
            code: None,
        }
    }

    /// 410: the token's account was deleted.
    pub fn account_deleted() -> ApiError {
        ApiError {
            code: Some("account_deleted"),
            ..ApiError::new(StatusCode::GONE, "This account was deleted")
        }
    }

    /// 422, with the fields that were wrong.
    pub fn from_field_errors(field_errors: FieldErrors) -> ApiError {
        ApiError {
            status: StatusCode::UNPROCESSABLE_ENTITY,
            message: "Some fields need another look".to_string(),
            field_errors: Some(field_errors),
            code: None,
        }
    }
}

impl From<PostError> for ApiError {
    fn from(error: PostError) -> Self {
        let status = match &error {
            PostError::NotFound { .. } => StatusCode::NOT_FOUND,
            PostError::NotOwner => StatusCode::FORBIDDEN,
            PostError::Validation(_) | PostError::Invalid(_) | PostError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            PostError::Community(error) => return ApiError::from_post_community(error),
            PostError::Record(error) => return ApiError::from_record(error),
            PostError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    fn from_post_community(error: &PostCommunityError) -> ApiError {
        let status = match error {
            PostCommunityError::UnknownPost { .. } | PostCommunityError::NotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            PostCommunityError::NotOwner | PostCommunityError::NotAMember { .. } => {
                StatusCode::FORBIDDEN
            }
            PostCommunityError::Invalid(_) | PostCommunityError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            PostCommunityError::Record(error) => return ApiError::from_record(error),
            PostCommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    fn from_file_community(error: &FileCommunityError) -> ApiError {
        let status = match error {
            FileCommunityError::UnknownFile { .. } | FileCommunityError::NotFound { .. } => {
                StatusCode::NOT_FOUND
            }
            FileCommunityError::NotAMember { .. } => StatusCode::FORBIDDEN,
            FileCommunityError::Invalid(_) | FileCommunityError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            FileCommunityError::Record(error) => return ApiError::from_record(error),
            FileCommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<FileCommunityError> for ApiError {
    fn from(error: FileCommunityError) -> Self {
        ApiError::from_file_community(&error)
    }
}

impl ApiError {
    /// A write the server did not record: 403 for a trial user who has
    /// made all the writes a trial may.
    fn from_record(error: &RecordError) -> ApiError {
        let status = match error {
            RecordError::TrialWritesFull => StatusCode::FORBIDDEN,
            RecordError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<PostCommunityError> for ApiError {
    fn from(error: PostCommunityError) -> Self {
        ApiError::from_post_community(&error)
    }
}

impl From<CommunityError> for ApiError {
    fn from(error: CommunityError) -> Self {
        let status = match &error {
            CommunityError::NotFound { .. } => StatusCode::NOT_FOUND,
            CommunityError::NotAMember => StatusCode::FORBIDDEN,
            CommunityError::Invalid(field_errors) => {
                return ApiError::from_field_errors(field_errors.clone());
            }
            CommunityError::Malformed(_) | CommunityError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            CommunityError::Membership(error) => return ApiError::from_membership(error),
            CommunityError::Record(error) => return ApiError::from_record(error),
            CommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    fn from_membership(error: &MembershipError) -> ApiError {
        let status = match error {
            MembershipError::NotFound { .. } | MembershipError::UnknownCommunity { .. } => {
                StatusCode::NOT_FOUND
            }
            MembershipError::Invalid(_) | MembershipError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            MembershipError::Record(error) => return ApiError::from_record(error),
            MembershipError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<MembershipError> for ApiError {
    fn from(error: MembershipError) -> Self {
        ApiError::from_membership(&error)
    }
}

impl From<CurrentCommunityError> for ApiError {
    fn from(error: CurrentCommunityError) -> Self {
        let status = match error {
            CurrentCommunityError::Community(error) => return error.into(),
            CurrentCommunityError::NotAMember => StatusCode::FORBIDDEN,
            CurrentCommunityError::DeviceOnly
            | CurrentCommunityError::DeviceSettings(_)
            | CurrentCommunityError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<UserError> for ApiError {
    fn from(error: UserError) -> Self {
        let status = match error {
            UserError::NotFound { .. } => StatusCode::NOT_FOUND,
            UserError::UnknownToken | UserError::NotSignedIn | UserError::LocalUser => {
                StatusCode::UNAUTHORIZED
            }
            UserError::AccountDeleted => return ApiError::account_deleted(),
            UserError::Validation(_) | UserError::Invalid(_) | UserError::Rejected(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            UserError::AccountUser(error) => return error.into(),
            UserError::Record(error) => return ApiError::from_record(&error),
            UserError::ServerOnly | UserError::DeviceSettings(_) | UserError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AccountUserError> for ApiError {
    fn from(error: AccountUserError) -> Self {
        let status = match error {
            AccountUserError::NotFound { .. } => StatusCode::NOT_FOUND,
            AccountUserError::InvalidEmail | AccountUserError::EmailTaken => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AccountUserError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AvatarError> for ApiError {
    fn from(error: AvatarError) -> Self {
        let status = match error {
            AvatarError::Invalid(field_errors) => return ApiError::from_field_errors(field_errors),
            AvatarError::User(error) => return error.into(),
            AvatarError::Sync(error) => return error.into(),
            AvatarError::NotYours | AvatarError::TrialUser => StatusCode::FORBIDDEN,
            AvatarError::NoAvatarBucket => StatusCode::SERVICE_UNAVAILABLE,
            AvatarError::UploadMissing
            | AvatarError::UploadExpired
            | AvatarError::NotTheImageType
            | AvatarError::TooLarge => StatusCode::UNPROCESSABLE_ENTITY,
            AvatarError::ServerOnly | AvatarError::Storage(_) | AvatarError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<FileError> for ApiError {
    fn from(error: FileError) -> Self {
        let status = match error {
            FileError::Invalid(field_errors) => return ApiError::from_field_errors(field_errors),
            FileError::NotFound { .. } => StatusCode::NOT_FOUND,
            FileError::Record(error) => return ApiError::from_record(&error),
            FileError::Community(error) => return ApiError::from_file_community(&error),
            FileError::NotUploaded | FileError::Rejected(_) | FileError::Malformed(_) => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            FileError::NoFileStore => StatusCode::SERVICE_UNAVAILABLE,
            FileError::Storage(_) => StatusCode::BAD_GATEWAY,
            FileError::Sync(error) => return (*error).into(),
            FileError::NotOnDevice => StatusCode::NOT_FOUND,
            FileError::DeviceOnly | FileError::Io(_) | FileError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AccountError> for ApiError {
    fn from(error: AccountError) -> Self {
        let status = match error {
            AccountError::User(error) => return error.into(),
            AccountError::AccountUser(error) => return error.into(),
            AccountError::Community(error) => return error.into(),
            AccountError::Sync(error) => return error.into(),
            AccountError::SignInExpired
            | AccountError::CodeDoesNotMatch
            | AccountError::UnknownDeviceCode => StatusCode::UNAUTHORIZED,
            AccountError::PasswordDoesNotMatch | AccountError::NotATrial => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            AccountError::TrialsFull => StatusCode::SERVICE_UNAVAILABLE,
            AccountError::PasswordResetNotAllowed => StatusCode::FORBIDDEN,
            AccountError::Credential(
                CredentialError::PasswordTooShort | CredentialError::PasswordTooLong,
            ) => StatusCode::UNPROCESSABLE_ENTITY,
            AccountError::Mail(_) => StatusCode::SERVICE_UNAVAILABLE,
            AccountError::ServerOnly
            | AccountError::DeviceOnly
            | AccountError::Credential(_)
            | AccountError::DeviceSettings(_)
            | AccountError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<DeletionError> for ApiError {
    fn from(error: DeletionError) -> Self {
        let status = match error {
            DeletionError::User(error) => return error.into(),
            DeletionError::AccountUser(error) => return error.into(),
            DeletionError::Post(error) => return error.into(),
            DeletionError::Membership(error) => return error.into(),
            DeletionError::Community(error) => return error.into(),
            // Never 401, which a device takes for a sign-out
            DeletionError::CodeDoesNotMatch | DeletionError::TrialHasNoCode => {
                StatusCode::UNPROCESSABLE_ENTITY
            }
            DeletionError::Mail(_) => StatusCode::SERVICE_UNAVAILABLE,
            DeletionError::ServerOnly | DeletionError::Refused(_) | DeletionError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<StoreTransactionError> for ApiError {
    fn from(error: StoreTransactionError) -> Self {
        let status = match &error {
            StoreTransactionError::AppStore(error) => return ApiError::from_app_store(error),
            StoreTransactionError::StoreNotSetUp => StatusCode::NOT_FOUND,
            StoreTransactionError::SomeoneElses | StoreTransactionError::NoAccount => {
                StatusCode::FORBIDDEN
            }
            StoreTransactionError::NeedsAccount
            | StoreTransactionError::PriceNotForSale
            | StoreTransactionError::AlreadyOwned
            | StoreTransactionError::AlreadySubscribed => StatusCode::UNPROCESSABLE_ENTITY,
            StoreTransactionError::ServerOnly | StoreTransactionError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<AppStoreNotificationError> for ApiError {
    fn from(error: AppStoreNotificationError) -> Self {
        match &error {
            AppStoreNotificationError::AppStore(error) => ApiError::from_app_store(error),
            AppStoreNotificationError::ServerOnly | AppStoreNotificationError::Database(_) => {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            }
        }
    }
}

impl ApiError {
    fn from_app_store(error: &AppStoreError) -> ApiError {
        let status = match error {
            AppStoreError::NotVerified(_) | AppStoreError::NotFound(_) => StatusCode::BAD_REQUEST,
            AppStoreError::Incomplete(_) | AppStoreError::Transport(_) => StatusCode::BAD_GATEWAY,
            AppStoreError::Settings(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<SyncError> for ApiError {
    fn from(error: SyncError) -> Self {
        match error {
            SyncError::Post(error) => error.into(),
            SyncError::PostCommunity(error) => error.into(),
            SyncError::File(error) => error.into(),
            SyncError::FileCommunity(error) => error.into(),
            SyncError::Community(error) => error.into(),
            SyncError::Membership(error) => error.into(),
            SyncError::User(error) => error.into(),
            SyncError::Unauthorized => ApiError::new(StatusCode::UNAUTHORIZED, error.to_string()),
            SyncError::AccountDeleted => ApiError::account_deleted(),
            SyncError::NotAMember { .. } => ApiError::new(StatusCode::FORBIDDEN, error.to_string()),
            SyncError::Transport(_) | SyncError::Refused(_) | SyncError::Malformed(_) => {
                ApiError::new(StatusCode::BAD_REQUEST, error.to_string())
            }
            SyncError::Database(_) | SyncError::DeviceSettings(_) | SyncError::Purge(_) => {
                ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
            }
        }
    }
}

impl From<StripeEventError> for ApiError {
    fn from(error: StripeEventError) -> Self {
        let status = match error {
            StripeEventError::BadSignature(_) | StripeEventError::Unreadable(_) => {
                StatusCode::BAD_REQUEST
            }
            StripeEventError::Stripe(error) => return ApiError::from_stripe(&error),
            StripeEventError::ServerOnly | StripeEventError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl ApiError {
    /// A call to Stripe that failed: 502, as the fault is past the server.
    fn from_stripe(error: &StripeError) -> ApiError {
        ApiError::new(StatusCode::BAD_GATEWAY, error.to_string())
    }
}

impl From<CheckoutError> for ApiError {
    fn from(error: CheckoutError) -> Self {
        let status = match &error {
            CheckoutError::NotFound { .. } => StatusCode::NOT_FOUND,
            CheckoutError::NeedsAccount => StatusCode::FORBIDDEN,
            CheckoutError::PriceNotForSale
            | CheckoutError::AlreadyOwned
            | CheckoutError::AlreadySubscribed => StatusCode::UNPROCESSABLE_ENTITY,
            CheckoutError::Stripe(error) => return ApiError::from_stripe(error),
            CheckoutError::ServerOnly | CheckoutError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<PlanError> for ApiError {
    fn from(error: PlanError) -> Self {
        ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
    }
}

impl From<SubscriptionError> for ApiError {
    fn from(error: SubscriptionError) -> Self {
        let status = match &error {
            SubscriptionError::NotFound { .. } => StatusCode::NOT_FOUND,
            SubscriptionError::Ended
            | SubscriptionError::OtherStore
            | SubscriptionError::PaymentFailed
            | SubscriptionError::Canceling
            | SubscriptionError::PriceNotForSale
            | SubscriptionError::SamePrice => StatusCode::UNPROCESSABLE_ENTITY,
            SubscriptionError::Stripe(error) => return ApiError::from_stripe(error),
            SubscriptionError::ServerOnly | SubscriptionError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<PurchaseError> for ApiError {
    fn from(error: PurchaseError) -> Self {
        let status = match &error {
            PurchaseError::NotFound { .. } => StatusCode::NOT_FOUND,
            PurchaseError::ServerOnly | PurchaseError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl From<ProductError> for ApiError {
    fn from(error: ProductError) -> Self {
        let status = match &error {
            ProductError::NotFound { .. } => StatusCode::NOT_FOUND,
            ProductError::Stripe(error) => return ApiError::from_stripe(error),
            ProductError::ServerOnly | ProductError::Database(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        ApiError::new(status, error.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!("{}", self.message);
        }
        let mut body = match self.field_errors {
            Some(field_errors) => json!({ "error": self.message, "errors": field_errors }),
            None => json!({ "error": self.message }),
        };
        if let Some(code) = self.code {
            body["code"] = json!(code);
        }
        (self.status, Json(body)).into_response()
    }
}
