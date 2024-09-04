use std::u32;

use near_sdk::{
    collections::{LookupMap as LookUpMapCollections, UnorderedSet},
    env,
    json_types::{U128, U64},
    log, near, require,
    store::{IterableMap, IterableSet, LookupSet},
    AccountId, BorshStorageKey, NearToken, PanicOnDefault, Promise,
};

#[near(serializers = [borsh, json])]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PostedNote {
    pub id: Option<U64>,
    pub title: String,
    pub body: String,
}

impl PostedNote {
    pub fn new(title: String, body: String, id: Option<U64>) -> Self {
        Self { title, body, id }
    }
}

#[near(serializers = [borsh, json])]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MoneyJar {
    pub amount: U128,
    pub id: U128,
}

impl MoneyJar {
    pub fn new(amount: U128, id: U128) -> Self {
        Self { amount, id }
    }
}

#[near]
#[derive(BorshStorageKey)]
pub enum StorageKey {
    NotesPerUser,
    JarsPerUser,
    Notes(AccountId),
    Jars(String),
    Managers,
    UserPoints,
}

// Define the contract structure
#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Contract {
    note_book: IterableMap<AccountId, IterableSet<PostedNote>>,
    note_book_collections:
        LookUpMapCollections<AccountId, UnorderedSet<PostedNote>>,
    jars_per_user: IterableMap<AccountId, IterableSet<MoneyJar>>,
    next_entry_id: Option<u64>,
    managers: LookupSet<AccountId>,
}

// Implement the contract structure
#[near]
impl Contract {
    #[init]
    pub fn new(managers: Vec<AccountId>) -> Self {
        let mut managers_set = LookupSet::new(StorageKey::Managers);

        managers.into_iter().for_each(|manager| {
            managers_set.insert(manager);
        });

        Self {
            note_book: IterableMap::new(StorageKey::NotesPerUser),
            note_book_collections: LookUpMapCollections::new(b"mm".to_vec()),
            jars_per_user: IterableMap::new(StorageKey::JarsPerUser),
            managers: managers_set,
            next_entry_id: None,
        }
    }

    #[payable]
    pub fn add_note(&mut self, title: String, body: String) {
        let account_id = env::predecessor_account_id();

        let next_entry_id = self.next_entry_id.unwrap_or(0);

        let note =
            PostedNote::new(title.clone(), body, Some(next_entry_id.into()));

        self.internal_add_note(
            account_id.clone(),
            &note,
            Some(env::attached_deposit().as_yoctonear()),
            next_entry_id,
        );
    }

    pub fn create_jar(&mut self, amount: U128, id: U128) {
        let account_id = env::predecessor_account_id();

        let jar = MoneyJar::new(amount, id);

        if let Some(jars) = self.jars_per_user.get_mut(&account_id) {
            jars.insert(jar.clone());
        } else {
            let key = format!("{}{}", id.0, account_id);
            println!("KEY: {key}");

            self.jars_per_user.insert(
                account_id.clone(),
                IterableSet::new(StorageKey::Jars(key)),
            );

            let jars = self.jars_per_user.get_mut(&account_id).unwrap();
            jars.insert(jar.clone());
        }

        log!("Created jar for user: {}", account_id);
    }

    pub fn create_jar_timestamp(&mut self, amount: U128, id: U128) {
        let account_id = env::predecessor_account_id();

        let jar = MoneyJar::new(amount, id);

        let mut temp_storage = IterableSet::new(StorageKey::Jars(format!(
            "{}",
            env::block_timestamp_ms()
        )));

        temp_storage.insert(jar.clone());

        let mut temp_storage_even = IterableSet::new(StorageKey::Jars(
            format!("{}", env::block_timestamp_ms()),
        ));

        temp_storage.iter().for_each(|jar| {
            if jar.amount.0 % 2 == 0 {
                temp_storage_even.insert(jar.clone());
            }
        });

        if let Some(jars) = self.jars_per_user.get_mut(&account_id) {
            jars.insert(jar.clone());
        } else {
            self.jars_per_user.insert(
                account_id.clone(),
                IterableSet::new(StorageKey::Jars(format!(
                    "{}{}{}",
                    id.0,
                    account_id,
                    env::block_timestamp_ms()
                ))),
            );

            let jars = self.jars_per_user.get_mut(&account_id).unwrap();

            temp_storage_even.iter().for_each(|jar| {
                jars.insert(jar.clone());
            });
        }
        log!("Created jar for user: {}", account_id);
    }

    pub fn get_jars(&self, account_id: AccountId) -> Vec<&MoneyJar> {
        let jars = self.jars_per_user.get(&account_id).unwrap();

        jars.iter().collect()
    }

    pub fn remove_all_notes(&mut self) {
        let account_id = env::predecessor_account_id();

        self.note_book.remove(&account_id);
    }

    pub fn remove_all_notes_correct(&mut self) {
        let account_id = env::predecessor_account_id();

        let mut removed = self
            .note_book
            .remove(&account_id)
            .unwrap_or_else(|| env::panic_str("No user found"));

        removed.clear();
    }

    pub fn add_note_collection(&mut self, title: String, body: String) {
        let account_id = env::predecessor_account_id();

        let next_entry_id = self.next_entry_id.unwrap_or(0);

        let note =
            PostedNote::new(title.clone(), body, Some(U64(next_entry_id)));

        self.internal_add_note_collection(&account_id, &note);

        self.next_entry_id = Some(next_entry_id + 1);

        log!("Added note to the note book: {}", note.title);
    }

    pub fn get_note(&self, account_id: AccountId, id: U64) -> &PostedNote {
        let id = id.0;

        require!(id <= self.next_entry_id.unwrap_or(0), "Note does not exist");

        let notes = self
            .note_book
            .get(&account_id)
            .unwrap_or_else(|| env::panic_str("no entry"));

        notes
            .iter()
            .find(|note| note.id.unwrap() == id.into())
            .unwrap_or_else(|| env::panic_str("no entry"))
    }

    pub fn get_notes(
        &self,
        account_id: AccountId,
        from_index: Option<u32>,
        limit: Option<u32>,
    ) -> Vec<&PostedNote> {
        let notes = self
            .note_book
            .get(&account_id)
            .unwrap_or_else(|| env::panic_str("no entry"));

        notes
            .iter()
            .skip(from_index.unwrap_or(0) as usize)
            .take(limit.unwrap_or(u32::MAX) as usize)
            .collect()
    }

    fn internal_add_note(
        &mut self,
        account_id: AccountId,
        note: &PostedNote,
        deposit: Option<u128>,
        next_entry_id: u64,
    ) {
        let storage_usage = env::storage_usage(); // storage before addition

        if let Some(notes) = self.note_book.get_mut(&account_id) {
            notes.insert(note.clone());
        } else {
            self.note_book.insert(
                account_id.clone(),
                IterableSet::new(StorageKey::Notes(account_id.clone())),
            );

            let notes = self.note_book.get_mut(&account_id).unwrap();
            notes.insert(note.clone());
        }

        self.next_entry_id = Some(next_entry_id + 1);

        let storage_after = env::storage_usage(); // storage after addition

        let storage_cost = env::storage_byte_cost().as_yoctonear()
            * (storage_after - storage_usage) as u128;

        let to_refund = deposit
            .unwrap_or(0)
            .checked_sub(storage_cost)
            .expect("not enough attached deposit");

        if to_refund != 0 {
            Promise::new(account_id)
                .transfer(NearToken::from_yoctonear(to_refund));
        }

        log!("Added note to the note book: {}", note.title);
    }

    fn internal_add_note_collection(
        &mut self,
        account_id: &AccountId,
        note: &PostedNote,
    ) {
        if let Some(mut notes) = self.note_book_collections.get(&account_id) {
            notes.insert(note);
        } else {
            self.note_book_collections.insert(
                account_id,
                &UnorderedSet::new(StorageKey::Notes(account_id.clone())),
            );

            let mut notes =
                self.note_book_collections.get(&account_id).unwrap();
            notes.insert(note);
        }
    }
}

#[cfg(test)]
mod tests {
    use near_sdk::{test_utils::VMContextBuilder, testing_env, NearToken};

    use super::*;

    #[test]
    fn add_note() {
        let mut contract =
            Contract::new(vec!["some_acc.near".parse().unwrap()]);

        let account_id = "account_id1";
        set_context(account_id, NearToken::from_near(1));

        let posted_note = PostedNote::new(
            "title".into(),
            "body".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract.add_note(posted_note.title.clone(), posted_note.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        assert_eq!(notes.len(), 1);
        assert!(notes.contains(&posted_note));

        // add another note for the same account
        let posted_note_2 = PostedNote::new(
            "title2".into(),
            "body2".into(),
            Some(contract.next_entry_id.unwrap().into()),
        );

        contract
            .add_note(posted_note_2.title.clone(), posted_note_2.body.clone());

        // add another note for a different account
        let account_id_2 = "account_id2";
        set_context(account_id_2, NearToken::from_near(1));

        let posted_note_3 = PostedNote::new(
            "title3".into(),
            "body3".into(),
            Some(contract.next_entry_id.unwrap().into()),
        );

        contract
            .add_note(posted_note_3.title.clone(), posted_note_3.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        let notes_2 = contract
            .note_book
            .get(&account_id_2.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        assert_eq!(notes.len(), 2);
        assert!(notes.contains(&posted_note));
        assert!(notes.contains(&posted_note_2));

        assert!(notes_2.contains(&posted_note_3));
    }

    #[test]
    fn add_note_collection() {
        let mut contract =
            Contract::new(vec!["some_acc.near".parse().unwrap()]);

        let account_id = "account_id1";
        set_context(account_id, NearToken::from_near(1));

        let posted_note = PostedNote::new(
            "title".into(),
            "body".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract.add_note_collection(
            posted_note.title.clone(),
            posted_note.body.clone(),
        );

        let notes = contract
            .note_book_collections
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        println!("NOTES STRUCTURE IS EMPTY {}", notes.is_empty());
        println!(
            "NOTES STRUCTURE CONTAINS NOTE {}",
            notes.contains(&posted_note)
        );

        // Len is zero but contains the posted note
        assert_eq!(notes.len(), 0);
        assert!(notes.contains(&posted_note));
    }
    #[test]
    fn remove_all_notes() {
        let mut contract =
            Contract::new(vec!["some_acc.near".parse().unwrap()]);

        let account_id = "account_id1";
        set_context(account_id, NearToken::from_near(1));

        let posted_note = PostedNote::new(
            "title".into(),
            "body".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract.add_note(posted_note.title.clone(), posted_note.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        assert_eq!(notes.len(), 1);
        assert!(notes.contains(&posted_note));

        contract.remove_all_notes();

        let posted_note2 = PostedNote::new(
            "title2".into(),
            "body2".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract
            .add_note(posted_note2.title.clone(), posted_note2.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        println!("NOTES LEN IS: {}", notes.len());
        println!(
            "NOTES STRUCTURE CONTAINS NOTE1: {}",
            notes.contains(&posted_note)
        );
        println!(
            "NOTES STRUCTURE CONTAINS NOTE2: {}",
            notes.contains(&posted_note2)
        );

        assert_eq!(notes.len(), 1);
        assert!(notes.contains(&posted_note));
        assert!(notes.contains(&posted_note2));
    }

    #[test]
    fn remove_all_notes_correct() {
        let mut contract =
            Contract::new(vec!["some_acc.near".parse().unwrap()]);

        let account_id = "account_id1";
        set_context(account_id, NearToken::from_near(1));

        let posted_note = PostedNote::new(
            "title".into(),
            "body".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract.add_note(posted_note.title.clone(), posted_note.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        assert_eq!(notes.len(), 1);
        assert!(notes.contains(&posted_note));

        contract.remove_all_notes_correct();

        let posted_note2 = PostedNote::new(
            "title2".into(),
            "body2".into(),
            Some(contract.next_entry_id.unwrap_or(0).into()),
        );

        contract
            .add_note(posted_note2.title.clone(), posted_note2.body.clone());

        let notes = contract
            .note_book
            .get(&account_id.to_string().parse::<AccountId>().unwrap())
            .unwrap();

        assert_eq!(notes.len(), 1);
        assert!(!notes.contains(&posted_note));
        assert!(notes.contains(&posted_note2));
    }

    fn set_context(predecessor: &str, amount: NearToken) {
        let mut builder = VMContextBuilder::new();
        builder.predecessor_account_id(predecessor.parse().unwrap());
        builder.attached_deposit(amount);

        testing_env!(builder.build());
    }
}
