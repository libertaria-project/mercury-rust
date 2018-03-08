extern crate futures;
extern crate multiaddr;
extern crate multihash;
extern crate tokio_core;
extern crate tokio_io;

use std::rc::Rc;

use futures::{Future, IntoFuture, Sink, Stream};
use futures::future;
use multiaddr::{Multiaddr};
use tokio_core::reactor;
use tokio_core::net::TcpStream;
use tokio_io::{AsyncRead, AsyncWrite};

pub mod imp;



// TODO
pub enum ErrorToBeSpecified { TODO, }



#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PublicKey(Vec<u8>);
#[derive(PartialEq, Eq, Clone, Debug, Copy)]
pub struct ProfileId(multihash::Hash);
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Signature(Vec<u8>);
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ApplicationId(String);
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct AppMessageFrame(Vec<u8>);

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PairingCertificate
{
    initiator_id:   ProfileId,
    acceptor_id:    ProfileId,
    initiator_sign: Signature,
    acceptor_sign:  Signature,
    // TODO is a nonce needed?
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct HomeInvitation
{
    home_id: ProfileId,
    voucher: String,
    signature: Signature,
    // TODO is a nonce needed?
}



#[derive(PartialEq, Eq, Clone, Debug)]
pub struct PersonaFacet
{
    homes: Vec<ProfileId>,
    // TODO and probably a lot more data
}


#[derive(PartialEq, Eq, Clone, Debug)]
pub struct HomeFacet
{
    addrs: Vec<Multiaddr>,
    // TODO and probably a lot more data
}



// NOTE Given for each SUPPORTED app, not currently available (checked in) app, checkins are managed differently
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct ApplicationFacet
{
    id: ApplicationId,
    // TODO and probably a lot more data
}


#[derive(PartialEq, Eq, Clone, Debug)]
pub struct RawFacet
{
    data: Vec<u8>, // TODO or maybe multicodec output?
}



#[derive(PartialEq, Eq, Clone, Debug)]
pub enum ProfileFacet
{
    Home(HomeFacet),
    Persona(PersonaFacet),
    Application(ApplicationFacet),
    Raw(String),
}


#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Profile
{
    id:         ProfileId,
    pub_key:    PublicKey,
    facets:     Vec<ProfileFacet>,
}

impl Profile
{
    pub fn new(id: &ProfileId, pub_key: &PublicKey, facets: &[ProfileFacet]) -> Self
        { Self{ id: id.to_owned(), pub_key: pub_key.to_owned(), facets: facets.to_owned() } }
}



#[derive(PartialEq, Eq, Clone, Debug)]
pub struct Contact
{
    profile:    Profile,
    proof:      PairingCertificate,
}

impl Contact
{
    fn new(profile: &Profile, proof: &PairingCertificate) -> Self
        { Self { profile: profile.clone(), proof: proof.clone() } }
}



#[derive(PartialEq, Eq, Clone, Debug)]
pub struct OwnProfileData
{
    profile:        Profile,
    private_data:   Vec<u8>, // TODO maybe multicodec output?
}

impl OwnProfileData
{
    pub fn new(profile: &Profile, private_data: &[u8]) -> Self
        { Self{ profile: profile.clone(), private_data: private_data.to_owned() } }
}



#[derive(PartialEq, Eq, Clone, Debug)]
pub struct SecretKey(Vec<u8>);

// NOTE implemented containing a SecretKey or something similar internally
pub trait Signer
{
    fn pub_key(&self) -> &PublicKey;
    // TODO the data Vec<u8> to be signed ideally will be the output from Mudlee's multicodec lib
    fn sign(&self, data: Vec<u8>) -> Signature;
}



pub struct OwnProfile
{
    profile: OwnProfileData,
    signer:  Rc<Signer>,
}




// Potentially a whole network of nodes with internal routing and sharding
pub trait ProfileRepo
{
    fn list(&self, /* TODO what filter criteria should we have here? */ ) ->
        Box< Stream<Item=Profile, Error=ErrorToBeSpecified> >;

    fn load(&self, id: &ProfileId) ->
        Box< Future<Item=Profile, Error=ErrorToBeSpecified> >;

    // NOTE should be more efficient than load(id) because URL is supposed to contain hints for resolution
    fn resolve(&self, url: &str) ->
        Box< Future<Item=Profile, Error=ErrorToBeSpecified> >;

    // TODO notifications on profile updates should be possible
}



pub struct CallMessages
{
    incoming: Box< Stream<Item=AppMessageFrame, Error=ErrorToBeSpecified> >,
    outgoing: Box< Sink<SinkItem=AppMessageFrame, SinkError=ErrorToBeSpecified> >,
}

pub struct Call
{
    caller:         ProfileId,
    init_payload:   AppMessageFrame,
    // NOTE A missed call will contain Option::None
    messages:       Option<CallMessages>,
}



// Interface to a single node
pub trait Home: ProfileRepo
{
    fn register(&self, prof: OwnProfile, invite: Option<HomeInvitation>) ->
        Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >;

    // TODO consider if we should notify an open session about an updated profile
    fn update(&self, profile: OwnProfile) ->
        Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >;

    // NOTE newhome is a profile that contains at least one HomeSchema different than this home
    fn unregister(&self, prof: OwnProfile, newhome: Option<Profile>) ->
        Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >;

    fn claim(&self, profile: Profile, signer: Rc<Signer>) ->
        Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >;


    // NOTE acceptor must have this server as its home
    fn pair_with(&self, initiator: &OwnProfile, acceptor: &Profile) ->
        Box< Future<Item=Contact, Error=ErrorToBeSpecified> >;

    fn call(&self, initiator: &OwnProfile, acceptor: &Contact,
            app: ApplicationId, init_payload: &[u8]) ->
        Box< Future<Item=CallMessages, Error=ErrorToBeSpecified> >;


    fn login(&self, profile: &OwnProfile) ->
        Box< Future<Item=Box<Session>, Error=ErrorToBeSpecified> >;
}



pub trait Session
{
    fn checkin_app(&self, app: &ApplicationId) ->
        Box< Stream<Item=Call, Error=ErrorToBeSpecified> >;

    fn checkout_app(&self, app: &ApplicationId, calls: Stream<Item=Call, Error=ErrorToBeSpecified>) ->
        Box< Future<Item=(), Error=ErrorToBeSpecified> >;


    fn banned_profiles(&self) ->
        Box< Future<Item=Vec<ProfileId>, Error=ErrorToBeSpecified> >;

    fn ban(&self, profile: &ProfileId) ->
        Box< Future<Item=(), Error=ErrorToBeSpecified> >;

    fn unban(&self, profile: &ProfileId) ->
        Box< Future<Item=(), Error=ErrorToBeSpecified> >;
}




pub trait HomeConnector
{
    // NOTE home_profile must have a HomeFacet with at least an address filled in
    fn connect(&self, home_profile: &Profile) ->
        Box< Future<Item=Rc<Home>, Error=ErrorToBeSpecified> >;
}



pub trait Client
{
    fn contacts(&self) -> Box< Stream<Item=Contact, Error=()> >;    // TODO error type
    fn profiles(&self) -> Box< Stream<Item=OwnProfile, Error=()> >; // TODO error type

    fn pair_with(&self, initiator: OwnProfile, acceptor_profile_url: &str) ->
        Box< Future<Item=Contact, Error=ErrorToBeSpecified> >;

    fn call(&self, contact: &Contact, app: &ApplicationId) ->
        Box< Future<Item=Call, Error=ErrorToBeSpecified> >;

    fn login(&self, profile: &OwnProfile) ->
        Box< Future<Item=Box<Session>, Error=ErrorToBeSpecified> >;
}



#[derive(Clone)]
pub struct ClientImp
{
    profile_repo:   Rc<ProfileRepo>,
    home_connector: Rc<HomeConnector>,
}


impl ClientImp
{
    fn pair_with(initiator: OwnProfile, acceptor_profile: Profile,
                 profile_repo: Rc<ProfileRepo>, home_connector: Rc<HomeConnector>) ->
        Box< Future<Item=Contact, Error=ErrorToBeSpecified> >
    {
        // Extract home ids from profile data
        let profile_clone = acceptor_profile.clone();
        let home_conn_futs = acceptor_profile.facets.iter()
            .flat_map( |facet|
            {
                match facet
                {
                    &ProfileFacet::Persona(ref persona) => persona.homes.clone(),
                    _ => Vec::new(),
                }
            } )
            .map( move |home_prof_id|
            {
                // Load profiles from home ids
                let home_connector_clone = home_connector.clone();
                profile_repo.load(&home_prof_id)
                    .and_then( move |home_prof|
                    {
                        // Connect to loaded homeprofile (Home of the user to pair with)
                        home_connector_clone.connect(&home_prof)
                    } )
            } );

        // Pick first successful home connection
        let contact_fut = future::select_ok( home_conn_futs )
            // Pair with targeted profile on the successful home connection
            .and_then( move |(home, _pending_futs)|
            {
                home.pair_with(&initiator, &profile_clone)
            } );

        Box::new(contact_fut)
    }
}


impl Client for ClientImp
{
    fn contacts(&self) -> Box< Stream<Item=Contact, Error=()> >
    {
        // TODO
        let (send, recv) = futures::sync::mpsc::channel(0);
        Box::new(recv)
    }


    fn profiles(&self) -> Box< Stream<Item=OwnProfile, Error=()> >
    {
        // TODO
        let (send, recv) = futures::sync::mpsc::channel(0);
        Box::new(recv)
    }


    fn pair_with(&self, initiator: OwnProfile, acceptor_profile_url: &str) ->
        Box< Future<Item=Contact, Error=ErrorToBeSpecified> >
    {
        let prof_repo_clone = self.profile_repo.clone();
        let home_connector_clone = self.home_connector.clone();

        let pair_fut = self.profile_repo
            .resolve(acceptor_profile_url)
            .and_then( move |profile|
            {
                ClientImp::pair_with(initiator, profile, prof_repo_clone, home_connector_clone)
            } );

        Box::new(pair_fut)
    }


    fn call(&self, contact: &Contact, app: &ApplicationId) ->
        Box< Future<Item=Call, Error=ErrorToBeSpecified> >
    {
        Box::new( future::err(ErrorToBeSpecified::TODO) )

//        let result = contact.profile.find_addresses()
//            .map_err( |e| ConnectToContactError::LookupFailed(e) )
//            .and_then( |addrs|
//                {
//                    for addr in addrs
//                        {
//                        }
//                    future::err( ConnectToContactError::ConnectFailed(ConnectAddressError::TODO) )
//                } );
//
//        Box::new(result)
    }


    fn login(&self, profile: &OwnProfile) ->
        Box< Future<Item=Box<Session>, Error=ErrorToBeSpecified> >
    {
        Box::new( future::err(ErrorToBeSpecified::TODO) )
    }
}


#[cfg(test)]
mod tests
{
    use super::*;
    use multiaddr::ToMultiaddr;


    struct TestSetup
    {
        reactor: reactor::Core,
    }

    impl TestSetup
    {
        fn new() -> Self
        {
            Self{ reactor: reactor::Core::new().unwrap() }
        }
    }


    #[test]
    fn test_something()
    {
//        // TODO assert!( result.TODO );
    }
}
