use paymaster_starknet::math::denormalize_felt;

use crate::command::balance::BalanceResult;

// Display relayers addresses and balances in a table
//
// Example:
// ----------------------------------------------------
// Relayer Address        |            Balance (STRK) |
// ----------------------------------------------------
// 0x0000...0000          |           1               |
// 0x0000...0001          |           2               |
// 0x0000...0002          |           3               |
// 0x0000...0003          |           4               |
// ----------------------------------------------------
//
pub fn display_table(results: &Vec<Result<BalanceResult, paymaster_starknet::Error>>, account_name: &str) {
    println!("\n{}", "_".repeat(77));
    println!("| {:^50} | {:^20} |", account_name, "Balance (STRK)");
    println!("|{}|{}|", "-".repeat(52), "-".repeat(22));

    for result in results {
        match result {
            Ok(relayer_balance) => {
                let addr_str = format!("{:x}", relayer_balance.address);
                let cropped_addr = if addr_str.len() > 8 {
                    format!("0x{}...{}", &addr_str[..4], &addr_str[addr_str.len() - 4..])
                } else {
                    format!("0x{}", addr_str)
                };
                println!("| {:<50} | {:<20} |", cropped_addr, format!("{}", denormalize_felt(relayer_balance.balance, 18)));
            },
            Err(e) => {
                println!("| {:<50} | {:<20} |", "Error", format!("Failed: {}", e));
            },
        }
    }
    println!("{}", "_".repeat(77));
}
