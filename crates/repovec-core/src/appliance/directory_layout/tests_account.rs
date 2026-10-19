//! Unit tests for the `passwd`-record parsing and validation seam.
//!
//! `systemd-sysusers` leaves a pre-existing account untouched, so these tests
//! pin the rules that stop an unsafe `repovec` account from reaching the live
//! pre-flight: the parser must read the whole record, and the validator must
//! refuse a root-owned uid, a non-contract home, and a login-capable shell.

use super::{
    account::{find_repovec_account, validate_repovec_account},
    live::LiveLayoutError,
};

fn parsed_account(line: &str) -> Result<super::account::RepovecAccount, String> {
    find_repovec_account(line).ok_or_else(|| format!("no repovec account in {line:?}"))
}

#[test]
fn find_repovec_account_parses_a_contract_correct_entry() {
    let sample = "root:x:0:0:root:/root:/bin/bash\nrepovec:x:998:996:repovec appliance service user:/var/lib/repovec:/usr/sbin/nologin\n";

    let account = parsed_account(sample).expect("repovec account");

    assert_eq!(account.uid, 998);
    assert_eq!(account.home, "/var/lib/repovec");
    assert_eq!(account.shell, "/usr/sbin/nologin");
    assert!(validate_repovec_account(&account).is_ok());
}

#[test]
fn find_repovec_account_returns_none_when_absent() {
    let sample = "root:x:0:0:root:/root:/bin/bash\n";

    assert!(find_repovec_account(sample).is_none());
}

#[test]
fn find_repovec_account_ignores_a_prefix_match_on_the_name() {
    // `repovec-backup` contains `repovec` as a prefix; the lookup compares the
    // whole name column so the earlier row cannot stand in for the account.
    let sample = "repovec-backup:x:997:997:backup:/srv/backup:/usr/sbin/nologin\n\
                  repovec:x:998:996:repovec appliance service user:/var/lib/repovec:/usr/sbin/nologin\n";

    let account = parsed_account(sample).expect("repovec account");

    assert_eq!(account.uid, 998);
}

#[test]
fn find_repovec_account_rejects_a_truncated_entry() {
    // A row without the home and shell columns cannot prove the account is
    // contract-correct, so it must not be mistaken for a provisioned account.
    let sample = "repovec:x:998:996:repovec appliance service user:/var/lib/repovec\n";

    assert!(find_repovec_account(sample).is_none());
}

#[test]
fn validate_repovec_account_rejects_a_root_owned_account() {
    // systemd-sysusers leaves existing accounts untouched, so a host that
    // already carried this entry would give UID 0 the data tree and run every
    // `User=repovec` unit as root.
    let account = parsed_account(
        "repovec:x:0:0:repovec appliance service user:/var/lib/repovec:/usr/sbin/nologin\n",
    )
    .expect("repovec account");

    assert!(matches!(
        validate_repovec_account(&account),
        Err(LiveLayoutError::RepovecUserIsRoot { uid: 0 })
    ));
    assert_eq!(LiveLayoutError::RepovecUserIsRoot { uid: 0 }.path(), None);
}

#[test]
fn validate_repovec_account_rejects_a_login_capable_shell() {
    let account = parsed_account(
        "repovec:x:998:996:repovec appliance service user:/var/lib/repovec:/bin/bash\n",
    )
    .expect("repovec account");

    let error = validate_repovec_account(&account).expect_err("shell violation");

    assert_eq!(error.to_string(), "the repovec user shell must be /usr/sbin/nologin: /bin/bash");
}

#[test]
fn validate_repovec_account_rejects_a_mismatched_home() {
    let account = parsed_account(
        "repovec:x:998:996:repovec appliance service user:/home/repovec:/usr/sbin/nologin\n",
    )
    .expect("repovec account");

    let error = validate_repovec_account(&account).expect_err("home violation");

    assert_eq!(error.to_string(), "the repovec user home must be /var/lib/repovec: /home/repovec");
    assert_eq!(error.path(), None);
}
