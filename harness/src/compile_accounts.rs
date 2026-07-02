//! Instruction <-> Transaction account compilation, with key deduplication,
//! privilege handling, and program account stubbing.

use {
    mollusk_svm_error::error::{MolluskError, MolluskPanic},
    solana_account::{Account, AccountSharedData, WritableAccount},
    solana_instruction::Instruction,
    solana_message::{LegacyMessage, Message, SanitizedMessage},
    solana_pubkey::Pubkey,
    std::collections::{HashMap, HashSet},
};

pub fn compile_accounts<'a>(
    instructions: &[Instruction],
    accounts: impl Iterator<Item = &'a (Pubkey, Account)>,
    fallback_accounts: &HashMap<Pubkey, Account>,
) -> (SanitizedMessage, Vec<(Pubkey, AccountSharedData)>) {
    let message = Message::new(instructions, None);
    let sanitized_message = SanitizedMessage::Legacy(LegacyMessage::new(message, &HashSet::new()));

    let accounts: Vec<_> = accounts.collect();
    let transaction_accounts = build_transaction_accounts(
        &sanitized_message,
        &accounts,
        instructions,
        fallback_accounts,
    );

    (sanitized_message, transaction_accounts)
}

pub fn transaction_accounts_for_sanitized_message<'a>(
    message: &SanitizedMessage,
    accounts: impl Iterator<Item = &'a (Pubkey, Account)>,
    fallback_accounts: &HashMap<Pubkey, Account>,
) -> Vec<(Pubkey, AccountSharedData)> {
    let accounts: Vec<_> = accounts.collect();
    let program_ids = message
        .program_instructions_iter()
        .map(|(program_id, _)| *program_id)
        .collect();
    build_transaction_accounts_with_program_ids(
        message,
        &accounts,
        &program_ids,
        fallback_accounts,
        None,
    )
}

fn build_transaction_accounts(
    message: &SanitizedMessage,
    accounts: &[&(Pubkey, Account)],
    all_instructions: &[Instruction],
    fallback_accounts: &HashMap<Pubkey, Account>,
) -> Vec<(Pubkey, AccountSharedData)> {
    let program_ids: HashSet<Pubkey> = all_instructions.iter().map(|ix| ix.program_id).collect();
    build_transaction_accounts_with_program_ids(
        message,
        accounts,
        &program_ids,
        fallback_accounts,
        Some(all_instructions),
    )
}

fn build_transaction_accounts_with_program_ids(
    message: &SanitizedMessage,
    accounts: &[&(Pubkey, Account)],
    program_ids: &HashSet<Pubkey>,
    fallback_accounts: &HashMap<Pubkey, Account>,
    all_instructions: Option<&[Instruction]>,
) -> Vec<(Pubkey, AccountSharedData)> {
    message
        .account_keys()
        .iter()
        .map(|key| {
            if program_ids.contains(key) {
                if let Some(provided_account) = accounts.iter().find(|(k, _)| k == key) {
                    return (*key, AccountSharedData::from(provided_account.1.clone()));
                }
                if let Some(fallback) = fallback_accounts.get(key) {
                    return (*key, AccountSharedData::from(fallback.clone()));
                }
                // This shouldn't happen if fallbacks are set up correctly.
                let mut program_account = Account::default();
                program_account.set_executable(true);
                return (*key, program_account.into());
            }

            if *key == solana_instructions_sysvar::ID {
                if let Some((_, provided_account)) = accounts.iter().find(|(k, _)| k == key) {
                    return (*key, AccountSharedData::from(provided_account.clone()));
                }
                if let Some(fallback) = fallback_accounts.get(key) {
                    return (*key, AccountSharedData::from(fallback.clone()));
                }
                let (_, account) = match all_instructions {
                    Some(all_instructions) => {
                        crate::instructions_sysvar::keyed_account(all_instructions.iter())
                    }
                    None => crate::instructions_sysvar::keyed_account(std::iter::empty()),
                };
                return (*key, account.into());
            }

            let account = accounts
                .iter()
                .find(|(k, _)| k == key)
                .map(|(_, a)| AccountSharedData::from(a.clone()))
                .or_else(|| {
                    fallback_accounts
                        .get(key)
                        .map(|a| AccountSharedData::from(a.clone()))
                })
                .or_panic_with(MolluskError::AccountMissing(key));

            (*key, account)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        solana_hash::Hash,
        solana_message::{compiled_instruction::CompiledInstruction, Message},
    };

    #[test]
    fn transaction_accounts_for_sanitized_message_preserves_message_key_order() {
        let payer = Pubkey::new_unique();
        let writable_account = Pubkey::new_unique();
        let readonly_account = Pubkey::new_unique();
        let program_id = Pubkey::new_unique();

        let message_account_keys = vec![payer, writable_account, readonly_account, program_id];
        let message = Message::new_with_compiled_instructions(
            1,
            0,
            2,
            message_account_keys.clone(),
            Hash::default(),
            vec![CompiledInstruction::new_from_raw_parts(
                3,
                vec![],
                vec![1, 2],
            )],
        );
        let sanitized_message =
            SanitizedMessage::Legacy(LegacyMessage::new(message, &HashSet::new()));

        let mut program_account = Account::default();
        program_account.set_executable(true);
        let fallback_accounts = HashMap::from([(program_id, program_account)]);
        let provided_accounts = [
            (payer, Account::default()),
            (writable_account, Account::default()),
            (readonly_account, Account::default()),
        ];

        let transaction_accounts = transaction_accounts_for_sanitized_message(
            &sanitized_message,
            provided_accounts.iter(),
            &fallback_accounts,
        );

        let transaction_account_keys = transaction_accounts
            .into_iter()
            .map(|(pubkey, _)| pubkey)
            .collect::<Vec<_>>();
        assert_eq!(transaction_account_keys, message_account_keys);
    }
}
