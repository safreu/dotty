mod manager;
pub use manager::{DotfileManager, DotfileManagerError, DotfileManagerLoadError};

mod plan;
pub use plan::{Plan, PlanExecutionError, PlanExecutor};

mod inspection;
pub use inspection::{InspectionError, ManagedEntryInspector};
