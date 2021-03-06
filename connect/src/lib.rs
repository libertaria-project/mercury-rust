extern crate failure;
extern crate futures;
extern crate jsonrpc_core;
extern crate jsonrpc_pubsub;
#[macro_use]
extern crate log;
extern crate mercury_home_protocol;
extern crate mercury_storage;
extern crate multiaddr;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_codec;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_uds;



pub mod profile;
pub mod error;
pub use error::{Error, ErrorKind};
pub mod net;
pub use net::SimpleTcpHomeConnector;
pub mod jsonrpc;
pub mod sdk;
pub mod service;

pub mod simple_profile_repo;
pub use simple_profile_repo::SimpleProfileRepo;



use std::rc::Rc;

use futures::prelude::*;

use mercury_home_protocol::*;
use mercury_storage::async::KeyValueStore;



#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct DAppPermission(Vec<u8>);


pub trait Contact
{
    fn proof(&self) -> &RelationProof;
    fn call(&self, init_payload: AppMessageFrame) -> AsyncResult<DAppCall,Error>;
}


pub struct DAppCall
{
    pub outgoing: AppMsgSink,
    pub incoming: AppMsgStream
}

//impl Drop for DAppCall
//    { fn drop(&mut self) { debug!("DAppCall was dropped"); } }



pub enum DAppEvent
{
    PairingResponse(Box<Contact>),
    Call(Box<IncomingCall>), // TODO wrap IncomingCall so as call.answer() could return a DAppCall directly
}



pub trait DAppEndpoint
{
    // NOTE this implicitly asks for user interaction (through UI) selecting a profile to be used with the app
    fn dapp_session(&self, app: &ApplicationId, authorization: Option<DAppPermission>)
        -> AsyncResult<Rc<DAppSession>, Error>;
}



// NOTE A specific DApp is logged in to the Connect Service with given details, e.g. a selected profile.
//      A DApp might have several sessions, e.g. running in the name of multiple profiles.
pub trait DAppSession
{
    // After the session was initialized, the profile is selected and can be queried any time
    fn selected_profile(&self) -> &ProfileId;

    // TODO merge these two operations using an optional profile argument
    fn contacts(&self) -> AsyncResult<Vec<Box<Contact>>, Error>;
    fn contacts_with_profile(&self, profile: &ProfileId, relation_type: Option<&str>)
        -> AsyncResult<Vec<Box<Contact>>, Error>;
    fn initiate_contact(&self, with_profile: &ProfileId) -> AsyncResult<(), Error>;

    fn app_storage(&self) -> AsyncResult<KeyValueStore<String,String>, Error>;

    fn checkin(&self) -> AsyncResult< Box< Stream<Item=DAppEvent, Error=()> >, Error>;
}
