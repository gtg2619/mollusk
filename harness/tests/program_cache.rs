use {
    mollusk_svm::{program::loader_keys, Mollusk},
    solana_program_runtime::loaded_programs::ProgramCacheEntryType,
    solana_pubkey::Pubkey,
};

const TOKEN_ELF: &[u8] = include_bytes!("../../programs/token/src/elf/token.so");

#[test]
fn added_programs_are_immediately_visible_by_default() {
    let mut mollusk = Mollusk::default();
    let program_id = Pubkey::new_unique();

    mollusk.add_program_with_loader_and_elf(&program_id, &loader_keys::LOADER_V2, TOKEN_ELF);

    let entry = mollusk
        .program_cache
        .load_program(&program_id)
        .expect("program cache entry");
    assert!(matches!(&entry.program, ProgramCacheEntryType::Loaded(_)));
}

#[test]
fn deployment_slot_programs_respect_warped_slot_visibility() {
    let mut mollusk = Mollusk::default();
    let program_id = Pubkey::new_unique();

    mollusk.add_program_with_loader_and_elf_and_deployment_slot(
        &program_id,
        &loader_keys::LOADER_V2,
        TOKEN_ELF,
        10,
    );

    mollusk.warp_to_slot(10);
    let entry = mollusk
        .program_cache
        .load_program(&program_id)
        .expect("delayed program cache entry");
    assert!(matches!(
        &entry.program,
        ProgramCacheEntryType::DelayVisibility
    ));

    mollusk.warp_to_slot(11);
    let entry = mollusk
        .program_cache
        .load_program(&program_id)
        .expect("visible program cache entry");
    assert!(matches!(&entry.program, ProgramCacheEntryType::Loaded(_)));
}
