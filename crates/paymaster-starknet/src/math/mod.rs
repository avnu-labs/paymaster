use starknet::core::types::Felt;

pub fn parse_units(amount: f64, decimals: u32) -> Felt {
    Felt::from(amount as u128 * 10_u128.pow(decimals))
}

pub fn format_units(amount: Felt, decimals: u32) -> f64 {
    let amount_u128: u128 = amount.try_into().unwrap_or(0);
    amount_u128 as f64 / 10_u128.pow(decimals) as f64
}

#[cfg(test)]
mod tests {
    use starknet::core::types::Felt;

    use super::*;

    #[test]
    fn test_parse_units() {
        let amount = 1.0;
        let decimals = 18;
        let result = parse_units(amount, decimals);
        assert_eq!(result, Felt::from(1000000000000000000u64));
    }

    #[test]
    fn test_format_units() {
        let amount = Felt::from(1000000000000000000u64);
        let decimals = 18;
        let result = format_units(amount, decimals);
        assert_eq!(result, 1.0);
    }
}
