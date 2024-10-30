use cw_storage_plus::Item;

use crate::msg::PaymentParams;

pub const CONTRACT_NAME: &str = "my-collection-manager";
pub const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const PAYMENT_PARAMS: Item<PaymentParams> = Item::new("payment_params");
