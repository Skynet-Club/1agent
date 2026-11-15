//! Computer-use / browser sandbox.

pub mod cdp;
pub mod credentials;
pub mod egress;
pub mod policy;
pub mod session;

pub use cdp::{CdpClient, CdpError};
pub use credentials::{SecretError, SecretStore, SecretSummary, normalize_origin, origin_of};
pub use egress::{EgressError, EgressPolicy, EgressProxy, is_global_ip};
pub use policy::{
    ACTION_CARD_TTL_MS, ActionAuthorization, ActionCard, ActionCardError, ActionCardStore,
    ActionRequest, CardStatus, StoredActionCard, authorize, authorize_request,
};
pub use session::{
    BrowserConfig, BrowserError, BrowserSession, ElementDescriptor, default_browser_root,
};
