mod manager;
pub use manager::{DotfileManager, DotfileManagerError, DotfileManagerLoadError};

mod transaction;
pub use transaction::{OperationError, OperationTransaction, TransactionCommitError};
