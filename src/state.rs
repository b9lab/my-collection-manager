use cw_storage_plus::Item;

use crate::msg::PaymentParams;

pub const PAYMENT_PARAMS: Item<PaymentParams> = Item::new("payment_params");
