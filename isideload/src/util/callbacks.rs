use std::pin::Pin;

use crate::{
    auth::apple_account::{TwoFactorCallbackParams, TwoFactorCallbackResponse},
    dev::certificates::DevelopmentCertificate,
};
use rootcause::prelude::*;

// don't care if anything's send in the very single threaded wasm
#[cfg(not(target_arch = "wasm32"))]
pub trait MaybeSend: Send {}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> MaybeSend for T {}

#[cfg(target_arch = "wasm32")]
pub trait MaybeSend {}

#[cfg(target_arch = "wasm32")]
impl<T> MaybeSend for T {}

pub trait TwoFactorCallback: Fn(TwoFactorCallbackParams) -> Self::Future + Send + Sync {
    type Future: Future<Output = Result<TwoFactorCallbackResponse, Report>> + MaybeSend;
}

impl<C, Fut> TwoFactorCallback for C
where
    C: Fn(TwoFactorCallbackParams) -> Fut + Send + Sync,
    Fut: Future<Output = Result<TwoFactorCallbackResponse, Report>> + MaybeSend,
{
    type Future = Fut;
}

pub trait MaxCertsCallback: Fn(Vec<DevelopmentCertificate>) -> Self::Future + Send + Sync {
    type Future: Future<Output = Result<Option<Vec<String>>, Report>> + MaybeSend;
}

impl<C, Fut> MaxCertsCallback for C
where
    C: Fn(Vec<DevelopmentCertificate>) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Option<Vec<String>>, Report>> + MaybeSend,
{
    type Future = Fut;
}

// annoyingly, we can't use MaybeSend here because it isn't an auto trait, so we have to do it manually
#[cfg(not(target_arch = "wasm32"))]
pub type MaxCertsCallbackFuture =
    Pin<Box<dyn Future<Output = Result<Option<Vec<String>>, Report>> + Send + 'static>>;

#[cfg(target_arch = "wasm32")]
pub type MaxCertsCallbackFuture =
    Pin<Box<dyn Future<Output = Result<Option<Vec<String>>, Report>> + 'static>>;

// Helper types for when it makes more sense to use a concrete boxed callback instead of a generic type parameter
pub type MaxCertsCallbackBox =
    Box<dyn Fn(Vec<DevelopmentCertificate>) -> MaxCertsCallbackFuture + Send + Sync + 'static>;
