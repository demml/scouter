// the message module is used as a generic route to insert messages
// We also have a duplicate route in the drift module for inserting drift records
// As we expand the MessageRecord enum it makes more sense to have a generic route for inserting messages
// We may revisit the drift route in the future to refactor it to use the message route
pub mod route;

pub use route::*;
