use fake::Dummy;
use serde::Serialize;

use crate::{common::zz_amount::ZzIAmount, domain::transaction::ZzTxEffect};

pub type ClientId = u16;

/// Represents the current state of a client's balance.
#[derive(Debug, Clone, PartialEq, Serialize, Dummy)]
pub struct ZzClientBalance {
    #[serde(rename = "client")]
    pub client_id: ClientId,
    pub available: ZzIAmount,
    pub held: ZzIAmount,
    pub total: ZzIAmount,
    pub locked: bool,
}

impl ZzClientBalance {
    /// Mutates the client's balance depending on the effect of a transaction
    ///
    /// # Panics
    ///
    /// Calling this function with locked = true will panic
    pub fn process_tx_effect(&mut self, effect: ZzTxEffect) {
        assert!(!self.locked, "Called process tx effect with locked Balance");

        fn apply(b: bool, cur: &mut ZzIAmount, other: &ZzIAmount) {
            if b {
                cur.add(other);
            } else {
                cur.sub(other);
            }
        }

        let amount = effect.amount.to_i_amount();

        if let Some(change_available) = effect.available {
            apply(change_available, &mut self.available, &amount);
        }

        if let Some(change_held) = effect.held {
            apply(change_held, &mut self.held, &amount);
        }

        self.locked |= effect.locked;
    }

    pub fn compute_total(&mut self) {
        let mut total = self.available.clone();
        total.add(&self.held);
        self.total = total;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::zz_amount::{ZzIAmount, ZzUAmount};
    use crate::domain::transaction::ZzTxEffect;

    fn make_uamount(val: u64) -> ZzUAmount {
        ZzUAmount::new(val.into(), 0).unwrap()
    }

    fn make_iamount(val: i64) -> ZzIAmount {
        ZzIAmount::new(val.into(), 0).unwrap()
    }

    fn make_tx_effect(
        amount: u64,
        available: Option<bool>,
        held: Option<bool>,
        locked: bool,
    ) -> ZzTxEffect {
        ZzTxEffect {
            amount: make_uamount(amount),
            available,
            held,
            locked,
        }
    }

    #[test]
    fn test_process_tx_effect_deposit() {
        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: false,
        };

        let effect = make_tx_effect(25, Some(true), None, false);
        balance.process_tx_effect(effect);

        assert_eq!(balance.available.to_string(), "125");
        assert_eq!(balance.held.to_string(), "50");
        // locked should remain false
        assert!(!balance.locked);
    }

    #[test]
    fn test_process_tx_effect_withdraw() {
        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: false,
        };

        let effect = make_tx_effect(30, Some(false), None, false);
        balance.process_tx_effect(effect);

        assert_eq!(balance.available.to_string(), "70");
        assert_eq!(balance.held.to_string(), "50");
    }

    #[test]
    fn test_process_tx_effect_held() {
        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: false,
        };

        let effect = make_tx_effect(20, None, Some(true), false);
        balance.process_tx_effect(effect);

        assert_eq!(balance.available.to_string(), "100");
        assert_eq!(balance.held.to_string(), "70");
    }

    #[test]
    #[should_panic(expected = "Called process tx effect with locked Balance")]
    fn test_process_tx_effect_locked_panics() {
        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: true,
        };

        let effect = make_tx_effect(10, Some(true), None, false);
        balance.process_tx_effect(effect);
    }

    #[test]
    fn test_process_tx_effect_locking() {
        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: false,
        };

        let effect = make_tx_effect(10, Some(true), None, true);
        balance.process_tx_effect(effect);
        assert!(balance.locked);
    }

    #[test]
    fn test_total_balance_is_calculated_correctly() {
        let expected_balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(150),
            locked: false,
        };

        let mut balance = ZzClientBalance {
            client_id: 1,
            available: make_iamount(100),
            held: make_iamount(50),
            total: make_iamount(0),
            locked: false,
        };

        balance.compute_total();

        assert_eq!(balance, expected_balance);
    }
}
