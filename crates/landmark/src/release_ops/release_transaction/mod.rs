mod bind;
mod commit;
mod confinement;
mod persist;
mod prepare;
mod types;
mod validate;

#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "policy_tests.rs"]
mod policy_tests;
pub(crate) use bind::*;
pub(crate) use commit::*;
pub(crate) use confinement::*;
pub(crate) use persist::*;
pub(crate) use prepare::*;
pub(crate) use types::*;
pub(crate) use validate::*;

use crate::*;

pub(crate) fn release_transaction(args: ReleaseTransactionArgs) -> Result<()> {
    match args.command {
        ReleaseTransactionCommand::Prepare(args) => prepare_release_transaction(args),
        ReleaseTransactionCommand::Commit(args) => commit_release_transaction(args),
        ReleaseTransactionCommand::Bind(args) => bind_release_transaction(args),
    }
}
