use starknet::core::types::Felt;

/// ServerAction enum matching the Cairo contract's `ServerAction` enum.
/// Cairo Serde serializes enums as `(variant_index, ...fields)`.
/// Variant ordering must match the Cairo definition in `actions.cairo`.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerAction {
    /// Variant 0: WriteOnce { storage_address: felt252, value: Span<felt252> }
    WriteOnce { storage_address: Felt, value: Vec<Felt> },
    /// Variant 1: Append { recipient_addr: ContractAddress, enc_channel_info: EncChannelInfo(3 felts) }
    Append { recipient_addr: Felt, enc_channel_info: [Felt; 3] },
    /// Variant 2: TransferFrom { from_addr: ContractAddress, token: ContractAddress, amount: u128 }
    TransferFrom { from_addr: Felt, token: Felt, amount: u128 },
    /// Variant 3: TransferTo { to_addr: ContractAddress, token: ContractAddress, amount: u128 }
    TransferTo { to_addr: Felt, token: Felt, amount: u128 },
    /// Variant 4: EmitViewingKeySet { user_addr: ContractAddress, public_key: felt252, enc_private_key: (3 felts) }
    EmitViewingKeySet {
        user_addr: Felt,
        public_key: Felt,
        enc_private_key: [Felt; 3],
    },
    /// Variant 5: EmitWithdrawal { enc_user_addr: EncUserAddr(3 felts), to_addr: ContractAddress, token: ContractAddress, amount: u128 }
    EmitWithdrawal {
        enc_user_addr: [Felt; 3],
        to_addr: Felt,
        token: Felt,
        amount: u128,
    },
    /// Variant 6: EmitDeposit { user_addr: ContractAddress, token: ContractAddress, amount: u128 }
    EmitDeposit { user_addr: Felt, token: Felt, amount: u128 },
    /// Variant 7: EmitOpenNoteCreated { enc_recipient_addr: EncUserAddr(3 felts), token: ContractAddress, note_id: felt252 }
    EmitOpenNoteCreated { enc_recipient_addr: [Felt; 3], token: Felt, note_id: Felt },
    /// Variant 8: EmitEncNoteCreated { note_id: felt252, packed_value: felt252 }
    EmitEncNoteCreated { note_id: Felt, packed_value: Felt },
    /// Variant 9: EmitNoteUsed { nullifier: felt252 }
    EmitNoteUsed { nullifier: Felt },
    /// Variant 10: Invoke { contract_address: ContractAddress, calldata: Span<felt252> }
    Invoke { contract_address: Felt, calldata: Vec<Felt> },
    /// Variant 11: InvokeWithComputation { contract_address: ContractAddress, calldata: Span<felt252> }
    InvokeWithComputation { contract_address: Felt, calldata: Vec<Felt> },
}

/// Error type for ServerAction parsing
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ServerActionError {
    #[error("calldata ended unexpectedly")]
    UnexpectedEnd,
    #[error("unknown ServerAction variant: {0}")]
    UnknownVariant(u64),
    #[error("span length exceeds remaining calldata")]
    InvalidSpanLength,
    #[error("felt value exceeds target integer range")]
    ValueOutOfRange,
    #[error("too many actions declared")]
    TooManyActions,
    #[error("trailing data after declared actions")]
    TrailingData,
    #[error("invalid screening attestation suffix variant: {0}")]
    InvalidScreeningSuffix(u64),
}

/// Cursor-based parser for Cairo-serialized data
struct Cursor<'a> {
    data: &'a [Felt],
    pos: usize,
}

impl<'a> Cursor<'a> {
    fn new(data: &'a [Felt]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    fn next(&mut self) -> Result<Felt, ServerActionError> {
        if self.pos >= self.data.len() {
            return Err(ServerActionError::UnexpectedEnd);
        }
        let val = self.data[self.pos];
        self.pos += 1;
        Ok(val)
    }

    fn next_u64(&mut self) -> Result<u64, ServerActionError> {
        let felt = self.next()?;
        crate::math::felt_to_u64(felt).map_err(|_| ServerActionError::ValueOutOfRange)
    }

    fn next_u128(&mut self) -> Result<u128, ServerActionError> {
        let felt = self.next()?;
        crate::math::felt_to_u128(felt).map_err(|_| ServerActionError::ValueOutOfRange)
    }

    fn next_span(&mut self) -> Result<Vec<Felt>, ServerActionError> {
        let len = self.next_u64()? as usize;
        if len > self.remaining() {
            return Err(ServerActionError::InvalidSpanLength);
        }
        let span = self.data[self.pos..self.pos + len].to_vec();
        self.pos += len;
        Ok(span)
    }

    fn next_array<const N: usize>(&mut self) -> Result<[Felt; N], ServerActionError> {
        let mut arr = [Felt::ZERO; N];
        for item in &mut arr {
            *item = self.next()?;
        }
        Ok(arr)
    }
}

fn parse_action(cursor: &mut Cursor) -> Result<ServerAction, ServerActionError> {
    let variant = cursor.next_u64()?;
    match variant {
        0 => {
            let storage_address = cursor.next()?;
            let value = cursor.next_span()?;
            Ok(ServerAction::WriteOnce { storage_address, value })
        },
        1 => {
            let recipient_addr = cursor.next()?;
            let enc_channel_info = cursor.next_array::<3>()?;
            Ok(ServerAction::Append {
                recipient_addr,
                enc_channel_info,
            })
        },
        2 => {
            let from_addr = cursor.next()?;
            let token = cursor.next()?;
            let amount = cursor.next_u128()?;
            Ok(ServerAction::TransferFrom { from_addr, token, amount })
        },
        3 => {
            let to_addr = cursor.next()?;
            let token = cursor.next()?;
            let amount = cursor.next_u128()?;
            Ok(ServerAction::TransferTo { to_addr, token, amount })
        },
        4 => {
            let user_addr = cursor.next()?;
            let public_key = cursor.next()?;
            let enc_private_key = cursor.next_array::<3>()?;
            Ok(ServerAction::EmitViewingKeySet {
                user_addr,
                public_key,
                enc_private_key,
            })
        },
        5 => {
            let enc_user_addr = cursor.next_array::<3>()?;
            let to_addr = cursor.next()?;
            let token = cursor.next()?;
            let amount = cursor.next_u128()?;
            Ok(ServerAction::EmitWithdrawal {
                enc_user_addr,
                to_addr,
                token,
                amount,
            })
        },
        6 => {
            let user_addr = cursor.next()?;
            let token = cursor.next()?;
            let amount = cursor.next_u128()?;
            Ok(ServerAction::EmitDeposit { user_addr, token, amount })
        },
        7 => {
            let enc_recipient_addr = cursor.next_array::<3>()?;
            let token = cursor.next()?;
            let note_id = cursor.next()?;
            Ok(ServerAction::EmitOpenNoteCreated {
                enc_recipient_addr,
                token,
                note_id,
            })
        },
        8 => {
            let note_id = cursor.next()?;
            let packed_value = cursor.next()?;
            Ok(ServerAction::EmitEncNoteCreated { note_id, packed_value })
        },
        9 => {
            let nullifier = cursor.next()?;
            Ok(ServerAction::EmitNoteUsed { nullifier })
        },
        10 => {
            let contract_address = cursor.next()?;
            let calldata = cursor.next_span()?;
            Ok(ServerAction::Invoke { contract_address, calldata })
        },
        11 => {
            let contract_address = cursor.next()?;
            let calldata = cursor.next_span()?;
            Ok(ServerAction::InvokeWithComputation { contract_address, calldata })
        },
        _ => Err(ServerActionError::UnknownVariant(variant)),
    }
}

/// Parse ServerActions from `apply_actions` calldata.
///
/// The `apply_actions` signature is `apply_actions(actions: Span<ServerAction>, screening:
/// Option<ScreeningAttestation>)`. Cairo serializes `Span<T>` as `[len, elem0, elem1, ...]`, so the
/// calldata starts with the span length followed by all serialized actions. The screening-capable
/// pool appends a trailing `Option<ScreeningAttestation>`; pre-screening pools append nothing (both
/// shapes are accepted here). See [`skip_screening_attestation`].
const MAX_ACTIONS: usize = 1024;

pub fn parse_server_actions(calldata: &[Felt]) -> Result<Vec<ServerAction>, ServerActionError> {
    let mut cursor = Cursor::new(calldata);

    // First felt is the span length (number of actions)
    let num_actions = cursor.next_u64()? as usize;

    if num_actions > MAX_ACTIONS {
        return Err(ServerActionError::TooManyActions);
    }

    let mut actions = Vec::with_capacity(num_actions);
    for _ in 0..num_actions {
        actions.push(parse_action(&mut cursor)?);
    }

    // Consume the optional trailing `Option<ScreeningAttestation>`. The paymaster only relays this
    // suffix verbatim (the contract verifies it); we just skip it so the gas-token transfer can
    // still be located. Pre-screening pools emit no suffix, so an empty remainder stays valid.
    if cursor.remaining() > 0 {
        skip_screening_attestation(&mut cursor)?;
    }

    if cursor.remaining() > 0 {
        return Err(ServerActionError::TrailingData);
    }

    Ok(actions)
}

/// Skip a trailing `Option<ScreeningAttestation>` (Cairo Serde encoding):
///   - `None` => `[0x1]`
///   - `Some(ScreeningAttestation { issued_at: u64, signature: (felt, felt) })`
///     => `[0x0, issued_at, sig_r, sig_s]`
fn skip_screening_attestation(cursor: &mut Cursor) -> Result<(), ServerActionError> {
    match cursor.next_u64()? {
        1 => Ok(()),                               // None
        0 => cursor.next_array::<3>().map(|_| ()), // Some: issued_at, sig_r, sig_s
        variant => Err(ServerActionError::InvalidScreeningSuffix(variant)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use starknet::macros::felt;

    #[test]
    fn parse_transfer_to() {
        let calldata = vec![
            Felt::ONE,           // 1 action
            Felt::THREE,         // variant 3 = TransferTo
            felt!("0xABC"),      // to_addr
            felt!("0xDEF"),      // token
            Felt::from(1000u64), // amount (u128 as single felt)
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::TransferTo {
                to_addr: felt!("0xABC"),
                token: felt!("0xDEF"),
                amount: 1000,
            }
        );
    }

    #[test]
    fn parse_write_once_variable_length() {
        let calldata = vec![
            Felt::ONE,        // 1 action
            Felt::ZERO,       // variant 0 = WriteOnce
            felt!("0x123"),   // storage_address
            Felt::from(3u64), // span length = 3
            felt!("0xA"),
            felt!("0xB"),
            felt!("0xC"),
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::WriteOnce {
                storage_address: felt!("0x123"),
                value: vec![felt!("0xA"), felt!("0xB"), felt!("0xC")],
            }
        );
    }

    #[test]
    fn parse_invoke_variable_length() {
        let calldata = vec![
            Felt::ONE,         // 1 action
            Felt::from(10u64), // variant 10 = Invoke
            felt!("0x456"),    // contract_address
            Felt::TWO,         // span length = 2
            felt!("0xD"),
            felt!("0xE"),
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::Invoke {
                contract_address: felt!("0x456"),
                calldata: vec![felt!("0xD"), felt!("0xE")],
            }
        );
    }

    #[test]
    fn parse_invoke_with_computation_variable_length() {
        let calldata = vec![
            Felt::ONE,         // 1 action
            Felt::from(11u64), // variant 11 = InvokeWithComputation
            felt!("0x456"),    // contract_address
            Felt::TWO,         // span length = 2
            felt!("0xD"),
            felt!("0xE"),
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::InvokeWithComputation {
                contract_address: felt!("0x456"),
                calldata: vec![felt!("0xD"), felt!("0xE")],
            }
        );
    }

    /// A compute-and-invoke `apply_actions` span carries an `InvokeWithComputation` action
    /// alongside the fee `TransferTo` that repays the relayer; both must decode so fee
    /// discovery can locate the transfer.
    #[test]
    fn parse_invoke_with_computation_alongside_fee_transfer() {
        let calldata = vec![
            Felt::TWO, // 2 actions
            // Action 0: InvokeWithComputation (variant 11)
            Felt::from(11u64),
            felt!("0x456"), // contract_address
            Felt::ONE,      // span length = 1
            felt!("0xD"),
            // Action 1: TransferTo (variant 3) — fee to forwarder
            Felt::THREE,
            felt!("0xFEE"),     // to_addr (forwarder)
            felt!("0x111"),     // token
            Felt::from(100u64), // amount
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], ServerAction::InvokeWithComputation { .. }));
        assert!(matches!(
            &actions[1],
            ServerAction::TransferTo { to_addr, amount, .. } if *to_addr == felt!("0xFEE") && *amount == 100
        ));
    }

    #[test]
    fn parse_emit_enc_note_created() {
        let calldata = vec![
            Felt::ONE,        // 1 action
            Felt::from(8u64), // variant 8 = EmitEncNoteCreated
            felt!("0xABC"),   // note_id
            felt!("0xDEF"),   // packed_value
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::EmitEncNoteCreated {
                note_id: felt!("0xABC"),
                packed_value: felt!("0xDEF")
            }
        );
    }

    #[test]
    fn parse_emit_note_used() {
        let calldata = vec![
            Felt::ONE,        // 1 action
            Felt::from(9u64), // variant 9 = EmitNoteUsed
            felt!("0x789"),   // nullifier
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], ServerAction::EmitNoteUsed { nullifier: felt!("0x789") });
    }

    #[test]
    fn parse_emit_withdrawal() {
        let calldata = vec![
            Felt::ONE,          // 1 action
            Felt::from(5u64),   // variant 5 = EmitWithdrawal
            felt!("0x11"),      // enc_user_addr[0] (auditor_public_key)
            felt!("0x22"),      // enc_user_addr[1] (ephemeral_pubkey)
            felt!("0x33"),      // enc_user_addr[2] (enc_user_addr)
            felt!("0xABC"),     // to_addr
            felt!("0xDEF"),     // token
            Felt::from(500u64), // amount
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0],
            ServerAction::EmitWithdrawal {
                enc_user_addr: [felt!("0x11"), felt!("0x22"), felt!("0x33")],
                to_addr: felt!("0xABC"),
                token: felt!("0xDEF"),
                amount: 500,
            }
        );
    }

    #[test]
    fn parse_multiple_actions() {
        // Simulate a Withdraw: TransferTo + EmitWithdrawal + WriteOnce (nullifier)
        let calldata = vec![
            Felt::from(3u64), // 3 actions
            // Action 0: TransferTo (variant 3)
            Felt::THREE,
            felt!("0xFEE"),     // to_addr (fee recipient)
            felt!("0x111"),     // token
            Felt::from(100u64), // amount
            // Action 1: EmitWithdrawal (variant 5)
            Felt::from(5u64),
            felt!("0xE1"),      // enc_user_addr[0]
            felt!("0xE2"),      // enc_user_addr[1]
            felt!("0xE3"),      // enc_user_addr[2]
            felt!("0xFEE"),     // to_addr
            felt!("0x111"),     // token
            Felt::from(100u64), // amount
            // Action 2: WriteOnce (variant 0)
            Felt::ZERO,
            felt!("0x999"),  // storage_address
            Felt::ONE,       // span length = 1
            felt!("0xDA1A"), // value
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert_eq!(actions.len(), 3);
        assert!(matches!(&actions[0], ServerAction::TransferTo { to_addr, amount, .. } if *to_addr == felt!("0xFEE") && *amount == 100));
        assert!(matches!(&actions[1], ServerAction::EmitWithdrawal { .. }));
        assert!(matches!(&actions[2], ServerAction::WriteOnce { .. }));
    }

    #[test]
    fn error_on_empty_calldata() {
        assert_eq!(parse_server_actions(&[]), Err(ServerActionError::UnexpectedEnd));
    }

    #[test]
    fn error_on_truncated_data() {
        let calldata = vec![
            Felt::ONE,   // 1 action expected
            Felt::THREE, // variant 3 = TransferTo
            felt!("0xABC"), // to_addr
                         // missing token and amount
        ];
        assert!(parse_server_actions(&calldata).is_err());
    }

    #[test]
    fn error_on_invalid_span_length() {
        let calldata = vec![
            Felt::ONE,          // 1 action
            Felt::ZERO,         // variant 0 = WriteOnce
            felt!("0x123"),     // storage_address
            Felt::from(999u64), // span length = 999 (way more than remaining)
        ];
        assert_eq!(parse_server_actions(&calldata), Err(ServerActionError::InvalidSpanLength));
    }

    #[test]
    fn error_on_unknown_variant() {
        let calldata = vec![
            Felt::ONE,         // 1 action
            Felt::from(99u64), // unknown variant
        ];
        assert_eq!(parse_server_actions(&calldata), Err(ServerActionError::UnknownVariant(99)));
    }

    #[test]
    fn parse_all_fixed_size_variants() {
        // Append (variant 1): 4 felts
        let calldata = vec![
            Felt::ONE,
            Felt::ONE, // variant 1
            felt!("0x1"),
            felt!("0x2"),
            felt!("0x3"),
            felt!("0x4"),
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert!(matches!(&actions[0], ServerAction::Append { .. }));

        // TransferFrom (variant 2): 3 felts
        let calldata = vec![
            Felt::ONE,
            Felt::TWO, // variant 2
            felt!("0x1"),
            felt!("0x2"),
            Felt::from(50u64),
        ];
        let actions = parse_server_actions(&calldata).unwrap();
        assert!(matches!(&actions[0], ServerAction::TransferFrom { amount: 50, .. }));

        // EmitViewingKeySet (variant 4): 5 felts
        let calldata = vec![Felt::ONE, Felt::from(4u64), felt!("0x1"), felt!("0x2"), felt!("0x3"), felt!("0x4"), felt!("0x5")];
        let actions = parse_server_actions(&calldata).unwrap();
        assert!(matches!(&actions[0], ServerAction::EmitViewingKeySet { .. }));

        // EmitDeposit (variant 6): 3 felts
        let calldata = vec![Felt::ONE, Felt::from(6u64), felt!("0x1"), felt!("0x2"), Felt::from(75u64)];
        let actions = parse_server_actions(&calldata).unwrap();
        assert!(matches!(&actions[0], ServerAction::EmitDeposit { amount: 75, .. }));

        // EmitOpenNoteCreated (variant 7): 5 felts (3 for EncUserAddr + token + note_id)
        let calldata = vec![Felt::ONE, Felt::from(7u64), felt!("0x1"), felt!("0x2"), felt!("0x3"), felt!("0x4"), felt!("0x5")];
        let actions = parse_server_actions(&calldata).unwrap();
        assert!(matches!(&actions[0], ServerAction::EmitOpenNoteCreated { .. }));
    }
}

/// Behaviour of [`parse_server_actions`] on the trailing `Option<ScreeningAttestation>`
/// suffix that the screening-capable pool appends to `apply_actions` calldata.
#[cfg(test)]
mod parse_server_actions_screening_suffix {
    use starknet::macros::felt;

    use super::*;

    /// Action span (TransferTo + WriteOnce) — the body shared by every case below.
    fn two_actions() -> Vec<Felt> {
        vec![
            Felt::TWO, // 2 actions
            // TransferTo (variant 3)
            Felt::THREE,
            felt!("0xFEE"),
            felt!("0x111"),
            Felt::from(100u64),
            // WriteOnce (variant 0)
            Felt::ZERO,
            felt!("0x999"),
            Felt::ONE,
            felt!("0xDA1A"),
        ]
    }

    #[test]
    fn should_parse_actions_when_suffix_is_option_none() {
        // given an action span followed by `Option::None` ([0x1]) from a non-deposit tx
        let mut calldata = two_actions();
        calldata.push(Felt::ONE);

        // when the calldata is parsed
        let actions = parse_server_actions(&calldata).unwrap();

        // then the actions are returned and the suffix is consumed
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[0], ServerAction::TransferTo { .. }));
        assert!(matches!(&actions[1], ServerAction::WriteOnce { .. }));
    }

    #[test]
    fn should_parse_actions_when_suffix_is_option_some() {
        // given an action span followed by `Option::Some(ScreeningAttestation)`
        // ([0x0, issued_at, sig_r, sig_s]) from a deposit tx
        let mut calldata = two_actions();
        calldata.extend([
            Felt::ZERO,                   // Some tag
            Felt::from(1_750_000_000u64), // issued_at
            felt!("0xAAA"),               // sig_r
            felt!("0xBBB"),               // sig_s
        ]);

        // when the calldata is parsed
        let actions = parse_server_actions(&calldata).unwrap();

        // then the actions are returned and the attestation is consumed
        assert_eq!(actions.len(), 2);
        assert!(matches!(&actions[1], ServerAction::WriteOnce { .. }));
    }

    #[test]
    fn should_error_when_suffix_variant_is_invalid() {
        // given a trailing felt that is neither None (1) nor Some (0)
        let mut calldata = two_actions();
        calldata.push(Felt::TWO);

        // when the calldata is parsed
        let result = parse_server_actions(&calldata);

        // then it is rejected as an invalid screening suffix
        assert_eq!(result, Err(ServerActionError::InvalidScreeningSuffix(2)));
    }

    #[test]
    fn should_error_when_some_suffix_is_truncated() {
        // given a `Some` suffix missing its signature felts
        let mut calldata = two_actions();
        calldata.extend([Felt::ZERO, Felt::from(1u64)]); // Some tag + issued_at only

        // when the calldata is parsed
        let result = parse_server_actions(&calldata);

        // then it is rejected as ended unexpectedly
        assert_eq!(result, Err(ServerActionError::UnexpectedEnd));
    }

    #[test]
    fn should_error_when_data_trails_after_suffix() {
        // given an extra felt after a complete `Option::None` suffix
        let mut calldata = two_actions();
        calldata.extend([Felt::ONE, felt!("0xBAD")]);

        // when the calldata is parsed
        let result = parse_server_actions(&calldata);

        // then the genuine trailing data is rejected
        assert_eq!(result, Err(ServerActionError::TrailingData));
    }
}
