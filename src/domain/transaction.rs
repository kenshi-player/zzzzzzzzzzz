use serde::{Deserialize, Serialize};
use strum::{EnumDiscriminants, IntoDiscriminant};

use std::collections::HashMap;

use crate::common::zz_amount::ZzUAmount;
use crate::domain::client_balance::{ClientId, ZzClientBalance};

pub type TxId = u32;

/// Represents a transaction in the system
#[derive(Debug, Clone, PartialEq, fake::Dummy)]
pub struct ZzTx {
    pub r#type: ZzTxType,
    pub client_id: ClientId,
    pub tx_id: TxId,
}

pub struct ZzTxSerializeCsv(pub ZzTx);

impl std::fmt::Display for ZzTxSerializeCsv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0.r#type {
            ZzTxType::Withdrawal(zz_amount) | ZzTxType::Deposit(zz_amount) => write!(
                f,
                "{},{},{},{}",
                self.0.r#type.discriminant(),
                self.0.client_id,
                self.0.tx_id,
                zz_amount
            ),
            _ => write!(
                f,
                "{},{},{}",
                self.0.r#type.discriminant(),
                self.0.client_id,
                self.0.tx_id
            ),
        }
    }
}

/// Represents the different kinds of transactions a client can have
#[derive(Debug, Clone, PartialEq, fake::Dummy, EnumDiscriminants)]
#[strum_discriminants(derive(Serialize, Deserialize), serde(rename_all = "kebab-case"))]
pub enum ZzTxType {
    Withdrawal(ZzUAmount),
    Deposit(ZzUAmount),
    Dispute,
    Resolve,
    Chargeback,
}

serde_plain::derive_display_from_serialize!(ZzTxTypeDiscriminants);

pub enum TransactionState {
    Deposit(ZzUAmount),
    Withdrawal,
    Dispute(ZzUAmount),
    Locked,
}

pub struct ZzTxEffect {
    pub amount: ZzUAmount,
    /// Some(true) -> increase by amount
    /// Some(false) -> decrease by amount
    /// None -> do nothing
    pub available: Option<bool>,
    /// Some(true) -> increase by amount
    /// Some(false) -> decrease by amount
    /// None -> do nothing
    pub held: Option<bool>,
    pub locked: bool,
}

pub trait TransactionMap {
    fn insert_transaction(
        &mut self,
        transaction: ZzTx,
        balance: Option<&ZzClientBalance>,
    ) -> Option<ZzTxEffect>;
}

/// This is an implementation of the transaction map using a hashmap
#[derive(Default)]
pub struct TransactionHashMapImpl {
    map: HashMap<(ClientId, TxId), TransactionState>,
}

impl TransactionMap for TransactionHashMapImpl {
    fn insert_transaction(
        &mut self,
        transaction: ZzTx,
        balance: Option<&ZzClientBalance>,
    ) -> Option<ZzTxEffect> {
        let tx_id = transaction.tx_id;
        let client_id = transaction.client_id;

        let (state, effect) =
            produce_effect(self.map.get(&(client_id, tx_id)), transaction, balance)?;

        self.map.insert((client_id, tx_id), state);
        Some(effect)
    }
}

// if an effect is produced, this means the transaction actually went through so it should be
// inserted and the effect should be returned
fn produce_effect(
    cur: Option<&TransactionState>,
    new: ZzTx,
    balance: Option<&ZzClientBalance>,
) -> Option<(TransactionState, ZzTxEffect)> {
    let balance_available = balance.map(|x| &x.available);

    if let Some(cur) = cur {
        match (cur, new.r#type) {
            (TransactionState::Deposit(zz_uint), ZzTxType::Dispute) => Some((
                TransactionState::Dispute(zz_uint.clone()),
                ZzTxEffect {
                    amount: zz_uint.clone(),
                    available: Some(false),
                    held: Some(true),
                    locked: false,
                },
            )),
            (TransactionState::Dispute(zz_uint), ZzTxType::Resolve) => Some((
                TransactionState::Deposit(zz_uint.clone()),
                ZzTxEffect {
                    amount: zz_uint.clone(),
                    available: Some(true),
                    held: Some(false),
                    locked: false,
                },
            )),
            (TransactionState::Dispute(zz_uint), ZzTxType::Chargeback) => Some((
                TransactionState::Locked,
                ZzTxEffect {
                    amount: zz_uint.clone(),
                    available: None,
                    held: Some(false),
                    locked: true,
                },
            )),
            _ => None,
        }
    } else {
        match new.r#type {
            ZzTxType::Withdrawal(zz_uint) => {
                if balance_available
                    .is_some_and(|available| available.greater_eq_than(zz_uint.clone()))
                {
                    Some((
                        TransactionState::Withdrawal,
                        ZzTxEffect {
                            amount: zz_uint,
                            available: Some(false),
                            held: None,
                            locked: false,
                        },
                    ))
                } else {
                    None
                }
            }
            ZzTxType::Deposit(zz_uint) => Some((
                TransactionState::Deposit(zz_uint.clone()),
                ZzTxEffect {
                    amount: zz_uint.clone(),
                    available: Some(true),
                    held: None,
                    locked: false,
                },
            )),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::zz_amount::{ZzIAmount, ZzUAmount};

    fn make_amount(val: u64) -> ZzUAmount {
        ZzUAmount::new(val.into(), 0).unwrap()
    }

    fn make_deposit_tx(client_id: ClientId, tx_id: TxId, amount: u64) -> ZzTx {
        ZzTx {
            r#type: ZzTxType::Deposit(make_amount(amount)),
            client_id,
            tx_id,
        }
    }

    fn make_withdraw_tx(client_id: ClientId, tx_id: TxId, amount: u64) -> ZzTx {
        ZzTx {
            r#type: ZzTxType::Withdrawal(make_amount(amount)),
            client_id,
            tx_id,
        }
    }

    fn make_dispute_tx(client_id: ClientId, tx_id: TxId) -> ZzTx {
        ZzTx {
            r#type: ZzTxType::Dispute,
            client_id,
            tx_id,
        }
    }

    fn make_resolve_tx(client_id: ClientId, tx_id: TxId) -> ZzTx {
        ZzTx {
            r#type: ZzTxType::Resolve,
            client_id,
            tx_id,
        }
    }

    fn make_chargeback_tx(client_id: ClientId, tx_id: TxId) -> ZzTx {
        ZzTx {
            r#type: ZzTxType::Chargeback,
            client_id,
            tx_id,
        }
    }

    #[test]
    fn test_insert_deposit() {
        let mut map = TransactionHashMapImpl {
            map: Default::default(),
        };
        let tx = make_deposit_tx(1, 100, 50);

        let effect = map.insert_transaction(tx, None).unwrap();
        assert_eq!(effect.amount.to_string(), "50");
        assert_eq!(effect.available, Some(true));
        assert_eq!(effect.held, None);
        assert!(!effect.locked);
    }

    #[test]
    fn test_insert_withdraw() {
        let mut map = TransactionHashMapImpl {
            map: Default::default(),
        };

        let tx = make_withdraw_tx(1, 101, 30);
        let effect = map
            .insert_transaction(
                tx,
                Some(&ZzClientBalance {
                    client_id: 1,
                    available: ZzIAmount::new(30.into(), 0).unwrap(),
                    held: ZzIAmount::zero(),
                    total: ZzIAmount::zero(),
                    locked: false,
                }),
            )
            .unwrap();
        assert_eq!(effect.amount.to_string(), "30");
        assert_eq!(effect.available, Some(false));
        assert_eq!(effect.held, None);
        assert!(!effect.locked);
    }

    #[test]
    fn test_insert_dispute_resolve_chargeback() {
        let mut map = TransactionHashMapImpl {
            map: Default::default(),
        };

        // Deposit first
        let deposit_tx = make_deposit_tx(1, 200, 100);
        map.insert_transaction(deposit_tx, None).unwrap();

        // Dispute
        let dispute_tx = make_dispute_tx(1, 200);
        let effect = map.insert_transaction(dispute_tx, None).unwrap();
        assert_eq!(effect.available, Some(false));
        assert_eq!(effect.held, Some(true));
        assert!(!effect.locked);

        // Resolve
        let resolve_tx = make_resolve_tx(1, 200);
        let effect = map.insert_transaction(resolve_tx, None).unwrap();
        assert_eq!(effect.available, Some(true));
        assert_eq!(effect.held, Some(false));
        assert!(!effect.locked);

        // Dispute again
        let dispute_tx = make_dispute_tx(1, 200);
        map.insert_transaction(dispute_tx, None).unwrap();

        // Chargeback
        let chargeback_tx = make_chargeback_tx(1, 200);
        let effect = map.insert_transaction(chargeback_tx, None).unwrap();
        assert_eq!(effect.available, None);
        assert_eq!(effect.held, Some(false));
        assert!(effect.locked);
    }

    #[test]
    fn test_invalid_dispute_or_resolve_on_nonexistent_tx() {
        let mut map = TransactionHashMapImpl {
            map: Default::default(),
        };

        // No prior transaction exists
        let dispute_tx = make_dispute_tx(1, 300);
        assert!(map.insert_transaction(dispute_tx, None).is_none());

        let resolve_tx = make_resolve_tx(1, 300);
        assert!(map.insert_transaction(resolve_tx, None).is_none());

        let chargeback_tx = make_chargeback_tx(1, 300);
        assert!(map.insert_transaction(chargeback_tx, None).is_none());
    }

    #[test]
    fn test_tx_with_wrong_client_id_produces_no_effect() {
        let mut map = TransactionHashMapImpl {
            map: Default::default(),
        };

        // Client 1 deposits
        let deposit_tx = make_deposit_tx(1, 400, 100);
        map.insert_transaction(deposit_tx, None).unwrap();

        // Client 1 disputes
        let dispute_tx = make_dispute_tx(1, 400);
        map.insert_transaction(dispute_tx, None).unwrap();

        // Now a chargeback arrives but with the wrong client_id (2 instead of 1)
        let chargeback_tx = make_chargeback_tx(2, 400);
        let effect = map.insert_transaction(chargeback_tx, None);

        // Because client_id mismatches, nothing should happen
        assert!(effect.is_none());

        // And the transaction map should still only contain the original client_id entry
        assert!(map.map.contains_key(&(1, 400)));
        assert!(!map.map.contains_key(&(2, 400)));
    }
}
