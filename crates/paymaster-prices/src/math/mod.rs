use bigdecimal::num_bigint::{BigInt, ToBigInt};
use bigdecimal::{BigDecimal, Zero};
use starknet::core::types::Felt;

use crate::{Error, TokenPrice};

pub fn convert_token_to_strk(token: &TokenPrice, amount: Felt) -> Result<Felt, Error> {
    let amount_scaled = BigDecimal::new(amount.to_bigint(), token.decimals);
    let price_scaled = BigDecimal::from_bigint(token.price_in_strk.to_bigint(), 0);

    let amount_in_strk_scaled = amount_scaled * price_scaled;
    let amount_in_strk = amount_in_strk_scaled;

    Ok(Felt::from((amount_in_strk).to_bigint().unwrap()))
}

pub fn convert_strk_to_token(token: &TokenPrice, amount: Felt, round_up: bool) -> Result<Felt, Error> {
    if token.price_in_strk.is_zero() {
        return Err(Error::InvalidPrice(token.price_in_strk));
    }

    let amount_scaled = BigDecimal::new(amount.to_bigint(), 18);
    let price_scaled = BigDecimal::from_bigint(token.price_in_strk.to_bigint(), 18);
    let amount_in_token_scaled = amount_scaled / price_scaled;

    let amount_in_token = amount_in_token_scaled * BigDecimal::from(10_u128.pow(token.decimals as u32));

    if round_up {
        let (rounded_int, _remainder) = amount_in_token.clone().with_scale(0).into_bigint_and_exponent();
        let remainder_check = &amount_in_token - BigDecimal::from(rounded_int.clone());
        let rounded = if remainder_check > BigDecimal::zero() {
            rounded_int + BigInt::from(1)
        } else {
            rounded_int
        };
        Ok(Felt::from(rounded))
    } else {
        Ok(Felt::from(amount_in_token.to_bigint().unwrap()))
    }
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use crate::math::{convert_strk_to_token, convert_token_to_strk};
    use crate::TokenPrice;

    #[cfg(test)]
    mod convert_strk_to_token {
        use starknet::macros::felt;

        use super::*;

        #[test]
        fn should_return_1_as_minimal_value_when_rounding_up() {
            // Given
            let amount = Felt::from(1);
            let wbtc_token_price = TokenPrice {
                address: felt!("0x3fe2b97c1fd336e750087d68b9b867997fd64a2661ff3ca5a7c771641e8e7ac"),
                decimals: 8,
                price_in_strk: felt!("0xcf12935faa2a43fbb200"),
            };

            // When
            let result = convert_strk_to_token(&wbtc_token_price, amount, true).unwrap();

            // Then
            assert_eq!(Felt::from(1), result);
        }

        #[test]
        fn should_return_rounding_up_value() {
            // Given
            let amount = Felt::from_dec_str("20000000000000000").unwrap();
            let wbtc_token_price = TokenPrice {
                address: felt!("0x3fe2b97c1fd336e750087d68b9b867997fd64a2661ff3ca5a7c771641e8e7ac"),
                decimals: 8,
                price_in_strk: felt!("0xcf12935faa2a43fbb200"),
            };

            // When
            let result = convert_strk_to_token(&wbtc_token_price, amount, true).unwrap();

            // Then
            assert_eq!(Felt::from(3), result);
        }
    }

    #[test]
    fn check_consistency() {
        let token = TokenPrice {
            address: Felt::ZERO,
            decimals: 8,
            price_in_strk: Felt::from((2000.0 * 1e17) as u128),
        };

        let amount_in = Felt::from_dec_str("100000000000000000").unwrap();

        let amount_out = convert_token_to_strk(&token, convert_strk_to_token(&token, amount_in, false).unwrap()).unwrap();
        assert_eq!(amount_in, amount_out);

        let amount_out = convert_strk_to_token(&token, convert_token_to_strk(&token, amount_in).unwrap(), false).unwrap();
        assert_eq!(amount_in, amount_out);
    }

    #[test]
    fn price_is_zero() {
        let token = TokenPrice {
            address: Felt::ZERO,
            decimals: 8,
            price_in_strk: Felt::ZERO,
        };

        let amount_in = Felt::from_dec_str("100000000000000000").unwrap();

        let amount_out = convert_strk_to_token(&token, amount_in, false);
        assert!(amount_out.is_err());

        let amount_out = convert_token_to_strk(&token, amount_in).unwrap();
        assert_eq!(amount_out, Felt::ZERO);
    }
}
