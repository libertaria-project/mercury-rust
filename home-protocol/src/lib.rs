extern crate bincode;
extern crate bytes;
extern crate capnp;
#[macro_use]
extern crate capnp_rpc;
extern crate ed25519_dalek;
#[macro_use]
extern crate failure;
extern crate futures;
#[macro_use]
extern crate log;
extern crate multiaddr;
extern crate multibase;
extern crate multihash;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate signatory;
extern crate signatory_dalek;
extern crate structopt;
extern crate tokio_core;
extern crate tokio_io;
extern crate toml;



pub mod crypto;
pub mod error;
pub mod future;
pub mod handshake;
pub mod mercury_capnp;
pub mod util;



use std::{rc::Rc, str};

use bincode::serialize;
use futures::{Future, sync::mpsc};
use multiaddr::{Multiaddr, ToMultiaddr};
use serde::{Deserialize, Deserializer, Serializer};
use serde::{de::Error as DeSerError, ser::SerializeSeq};

use crypto::{ProfileValidator, SignatureValidator};
use ::error::*;



pub const CHANNEL_CAPACITY: usize = 1;


#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct ProfileId(pub Vec<u8>); // NOTE multihash::encode() output

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct PublicKey(pub Vec<u8>);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct PrivateKey(pub Vec<u8>);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct Signature(pub Vec<u8>);



/// Something that can sign data, but cannot give out the private key.
/// Usually implemented using a private key internally, but also enables hardware wallets.
pub trait Signer
{
    fn profile_id(&self) -> &ProfileId;

    fn public_key(&self) -> &PublicKey;
    // NOTE the data to be signed ideally will be the output from Mudlee's multicodec lib
    fn sign(&self, data: &[u8]) -> Signature;
}



#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct PersonaFacet
{
    /// `homes` contain items with `relation_type` "home", with proofs included.
    /// Current implementation supports only a single home stored in `homes[0]`,
    /// Support for multiple homes will be implemented in a future release.
    pub homes:  Vec<RelationProof>,
    pub data:   Vec<u8>,
}



#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct HomeFacet
{
    /// Addresses of the same home server. A typical scenario of multiple addresses is when there is
    /// one IPv4 address/port, one onion address/port and some IPv6 address/port pairs.
    #[serde(serialize_with = "serialize_multiaddr_vec")]
    #[serde(deserialize_with = "deserialize_multiaddr_vec")]
    pub addrs:  Vec<Multiaddr>,
    pub data:   Vec<u8>,
}

// NOTE Given for each SUPPORTED app, not currently available (checked in) app, checkins are managed differently
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct ApplicationFacet
{
    /// unique id of the application - like 'iop-chat'
    pub id:     ApplicationId,
    pub data:   Vec<u8>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct RawFacet
{
    pub data: Vec<u8>, // TODO or maybe multicodec output?
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum ProfileFacet
{
    Home(HomeFacet),
    Persona(PersonaFacet),
    Application(ApplicationFacet),
    Unknown(RawFacet),
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct Profile
{
    /// The Profile ID is a hash of the public key, similar to cryptocurrency addresses.
    pub id:         ProfileId,

    /// Public key used for validating the identity of the profile.
    pub public_key: PublicKey,
    pub facet:      ProfileFacet, // TODO consider redesigning facet Rust types/storage
    // TODO consider having a signature of the profile data here
}



/// Represents a connection to another Profile (Home <-> Persona), (Persona <-> Persona)
#[derive(Clone)]
pub struct PeerContext
{
    my_signer: Rc<Signer>,
    peer_pubkey: PublicKey,
    peer_id: ProfileId,
}



pub type AsyncResult<T,E> = Box< Future<Item=T, Error=E> >;

pub type AsyncStream<Elem, RemoteErr> = mpsc::Receiver< std::result::Result<Elem, RemoteErr> >;
pub type AsyncSink<Elem, RemoteErr>   = mpsc::Sender< std::result::Result<Elem, RemoteErr> >;

/// Potentially a whole network of nodes with internal routing and sharding
pub trait ProfileRepo
{
//    /// List all profiles that can be load()'ed or resolve()'d.
//    fn list(&self, /* TODO what filter criteria should we have here? */ ) ->
//        HomeStream<Profile,String>;

    /// Look for specified `id` and return. This might involve searching for the latest version
    /// of the profile in the dht, but if it's the profile's home server, could come from memory, too.
    fn load(&self, id: &ProfileId) -> AsyncResult<Profile, Error>;

//    /// Same as load(), but also contains hints for resolution, therefore it's more efficient than load(id)
//    ///
//    /// The `url` may contain
//    /// * ProfileID (mandatory)
//    /// * some profile metadata (for user experience enhancement) (big fat warning should be thrown if it does not match the latest info)
//    /// * ProfileID of its home server
//    /// * last known multiaddress(es) of its home server
//    fn resolve(&self, url: &str) ->
//        AsyncResult<Profile, Error>;

    // TODO notifications on profile updates should be possible
}



#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct OwnProfile
{
    /// The public part of the profile. In the current implementation it must contain a single PersonaFacet.
    pub profile:    Profile,

    /// Hierarchical, json-like data structure, encoded using multicodec library,
    /// encrypted with the persona's keys, and stored on the home server
    pub priv_data:  Vec<u8>, // TODO maybe multicodec output?
}

impl OwnProfile
{
    pub fn new(profile: &Profile, private_data: &[u8]) -> Self
        { Self{ profile: profile.clone(), priv_data: private_data.to_owned() } }
}



// NOTE the binary blob to be signed is rust-specific: Strings are serialized to a u64 (size) and the encoded string itself.
// TODO consider if this is platform-agnostic enough, especially when combined with capnproto
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct RelationSignablePart {
    pub relation_type: String,
    pub signer_id: ProfileId,
    pub peer_id: ProfileId,
    // TODO is a nonce needed?
}



#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct RelationHalfProof
{
    pub relation_type:  String,
    pub signer_id:      ProfileId,
    pub peer_id:        ProfileId,
    pub signature:      Signature,
    // TODO is a nonce needed?
}

impl RelationHalfProof
{
    pub fn new(relation_type: &str, peer_id: &ProfileId, signer: &Signer) -> Self
    {
        let mut result = Self{ relation_type: relation_type.to_owned(),
                               signer_id: signer.profile_id().to_owned(),
                               peer_id: peer_id.to_owned(),
                               signature: Signature( Vec::new() ) };
        result.signature = RelationSignablePart::from(&result).sign(signer);
        result
    }
}


#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct RelationProof
{
    pub relation_type:  String,        // TODO inline halfproof fields with macro, if possible at all
    pub a_id:           ProfileId,
    pub a_signature:    Signature,
    pub b_id:           ProfileId,
    pub b_signature:    Signature,
    // TODO is a nonce needed?
}



/// This invitation allows a persona to register on the specified home.
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct HomeInvitation
{
    pub home_id:    ProfileId,

    /// A unique string that identifies the invitation
    pub voucher:    String,

    /// The signature of the home
    pub signature:  Signature,
    // TODO is a nonce needed?
    // TODO is an expiration time needed?
}

impl HomeInvitation
{
    pub fn new(home_id: &ProfileId, voucher: &str, signature: &Signature) -> Self
        { Self{ home_id: home_id.to_owned(), voucher: voucher.to_owned(), signature: signature.to_owned() } }
}


#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct ApplicationId(pub String);

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub struct AppMessageFrame(pub Vec<u8>);


pub type AppMsgStream = AsyncStream<AppMessageFrame, String>;
pub type AppMsgSink   = AsyncSink<AppMessageFrame, String>;


/// A struct that is passed from the caller to the callee. The callee can examine this
/// before answering the call.
#[derive(Debug)]
pub struct CallRequestDetails
{
    /// Proof for the home server that the caller is authorized to call the callee.
    /// The callee can find out who's calling by looking at `relation`.
    pub relation:       RelationProof,

    /// A message that the callee can examine before answering or rejecting a call. Note that the caller is already
    /// known to the callee through `relation`.
    pub init_payload:   AppMessageFrame,

    /// The sink half of a channel that routes `AppMessageFrame`s back to the caller. If the caller
    /// does not want to receive any response messages from the callee, `to_caller` should be set to `None`.
    pub to_caller:      Option<AppMsgSink>,
}


// Interface to a single home server.
// NOTE authentication is already done when the connection is built,
//      authenticated profile info is available from the connection context
pub trait Home: ProfileRepo
{
    // NOTE because we support multihash, the id cannot be guessed from the public key
    fn claim(&self, profile: ProfileId) -> AsyncResult<OwnProfile, Error>;

    // TODO this should return only the signed RelationProof of the home hosting the profile
    //      because in this form the home can return malicious changes in the profile
    fn register(&self, own_prof: OwnProfile, half_proof: RelationHalfProof, invite: Option<HomeInvitation>) ->
        AsyncResult<OwnProfile, (OwnProfile,Error)>;

    /// By calling this method, any active session of the same profile is closed.
    fn login(&self, proof_of_home: &RelationProof) -> AsyncResult<Rc<HomeSession>, Error>;

    /// The peer in `half_proof` must be hosted on this home server.
    /// Returns Error if the peer is not hosted on this home server or an empty result if it is.
    /// Note that the peer will directly invoke `pair_response` on the initiator's home server and call pair_response to send PairingResponse event
    fn pair_request(&self, half_proof: RelationHalfProof) -> AsyncResult<(), Error>;

    fn pair_response(&self, rel: RelationProof) -> AsyncResult<(), Error>;

    // NOTE initiating a real P2P connection (vs a single frame push notification),
    //      the caller must fill in some message channel to itself.
    //      A successful call returns a channel to callee.
    fn call(&self, app: ApplicationId, call_req: CallRequestDetails) ->
        AsyncResult<Option<AppMsgSink>, Error>;

// TODO consider how to do this in a later milestone
//    fn presence(&self, rel: Relation, app: ApplicationId) ->
//        AsyncResult<Option<AppMessageFrame>, Error>;
}


#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, PartialOrd, Serialize)]
pub enum ProfileEvent
{
    Unknown(Vec<u8>), // forward compatibility for protocol extension
    PairingRequest(RelationHalfProof),
    // TODO do we want to distinguish "rejected" and "notYetApproved" states for pairing, i.e. need an explicit rejected response?
    PairingResponse(RelationProof),
// TODO are these events needed? What others?
//    HomeBroadcast,
//    HomeHostingExpiry,
//    ProfileUpdated, // from a different client instance/session
}


pub trait IncomingCall
{
    /// Get a reference to details of the call.
    /// It contains information about the caller party (`relation`), an initial message (`initial_payload`)
    /// If the caller wishes to receive App messages from the calee, a sink should be passed in `to_caller`.
    fn request_details(&self) -> &CallRequestDetails;

    // NOTE this assumes boxed trait objects, if Rc of something else is needed, this must be revised
    // TODO consider offering the possibility to somehow send back a single AppMessageFrame
    //      as a reply to init_payload without a to_callee sink,
    //      either included into this function or an additional method
    /// Indicate to the caller that the call was answered.
    /// If the callee wishes to receive messages from the caller, it has to create a channel
    /// and pass the created sink to `answer()`, which is returned by `call()` on the caller side.
    fn answer(self: Box<Self>, to_callee: Option<AppMsgSink>) -> CallRequestDetails;
}


pub trait HomeSession
{
    fn update(&self, own_prof: OwnProfile) -> AsyncResult<(), Error>;

    // NOTE newhome is a profile that contains at least one HomeFacet different than this home
    // TODO should we return a modified OwnProfile here with this home removed from the homes of persona facet in profile?
    fn unregister(&self, newhome: Option<Profile>) -> AsyncResult<(), Error>;


    fn events(&self) -> AsyncStream<ProfileEvent, String>;

    // TODO some kind of proof might be needed that the AppId given really belongs to the caller
    // TODO add argument in a later milestone, presence: Option<AppMessageFrame>) ->
    fn checkin_app(&self, app: &ApplicationId) -> AsyncStream<Box<IncomingCall>, String>;

    // TODO remove this after testing
    fn ping(&self, txt: &str) -> AsyncResult<String, Error>;


// TODO ban features are delayed to a later milestone
//    fn banned_profiles(&self) -> AsyncResult<Vec<ProfileId>, Error>;
//    fn ban(&self, profile: &ProfileId) -> AsyncResult<(), Error>;
//    fn unban(&self, profile: &ProfileId) -> AsyncResult<(), Error>;
}



// ----------------------------------------------------------------------------------
// NOTE Following from here this is Rust technical stuff that must be defined
//      in this module to work in Rust but otherwise not strictly part of the protocol
// ----------------------------------------------------------------------------------

// NOTE this is identical to the currently experimental std::convert::TryFrom.
//      Hopefully this will not be needed soon when it stabilizes.
pub trait TryFrom<T> : Sized {
    type Error;
    fn try_from(value: T) -> Result<Self, Self::Error>;
}


impl<'a> From<&'a [u8]> for ProfileId
{
    fn from(src: &'a [u8]) -> Self
        { ProfileId( src.to_owned() ) }
}

impl<'a> From<&'a ProfileId> for &'a [u8]
{
    fn from(src: &'a ProfileId) -> Self
        { &src.0 }
}

impl<'a> From<ProfileId> for Vec<u8>
{
    fn from(src: ProfileId) -> Self
        { src.0 }
}


impl<'a> TryFrom<&'a str> for ProfileId
{
    type Error = ::multibase::Error;
    fn try_from(src: &'a str) -> Result<Self, Self::Error> {
        let (_base, binary) = ::multibase::decode(src)?;
        Ok( ProfileId(binary) )
    }
}

impl<'a> From<&'a ProfileId> for String
{
    fn from(src: &'a ProfileId) -> Self
        { ::multibase::encode(::multibase::Base::Base64url, &src.0) }
}

impl<'a> From<ProfileId> for String
{
    fn from(src: ProfileId) -> Self
        { Self::from(&src) }
}

impl std::fmt::Display for ProfileId {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}


impl<'a> From<&'a PublicKey> for String
{
    fn from(src: &'a PublicKey) -> Self
        { ::multibase::encode(::multibase::Base::Base64url, &src.0) }
}

impl<'a> From<PublicKey> for String
{
    fn from(src: PublicKey) -> Self
        { Self::from(&src) }
}

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", String::from(self))
    }
}



fn serialize_multiaddr_vec<S>(x: &Vec<Multiaddr>, s: S) -> std::result::Result<S::Ok, S::Error>
    where S: Serializer,
{
    let mut seq = s.serialize_seq(Some(x.len()))?;
    for mr in x{
        match seq.serialize_element(&mr.to_string()){
            Ok(_)=>{();},
            Err(e)=>{return Err(e);}
        }
    }
    seq.end()
}

fn deserialize_multiaddr_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<Multiaddr>, D::Error>
    where D: Deserializer<'de>,
{
    let mapped: Vec<String> = Deserialize::deserialize(deserializer)?;
    let mut res = Vec::new();
    for str_ma in mapped.iter(){;
        match str_ma.to_multiaddr(){
            Ok(multi)=>{res.push(multi);}
            Err(e)=>{return Err(D::Error::custom(e));}
        }
    }
    Ok(res)
}



impl Profile
{
    pub fn new(id: &ProfileId, public_key: &PublicKey, facet: &ProfileFacet) -> Self
        { Self{ id: id.to_owned(), public_key: public_key.to_owned(), facet: facet.to_owned() } }

    pub fn new_home(id: ProfileId, public_key: PublicKey, address: Multiaddr) -> Self
    {
        let facet = HomeFacet {
            addrs: vec![address],
            data: vec![],
        };

        Self { id, public_key, facet: ProfileFacet::Home(facet) }
    }
}



impl PeerContext
{
    pub fn new(my_signer: Rc<Signer>, peer_pubkey: PublicKey, peer_id: ProfileId) -> Self
        { Self{my_signer, peer_pubkey, peer_id} }
    pub fn new_from_profile(my_signer: Rc<Signer>, peer: &Profile) -> Self
        { Self::new( my_signer, peer.public_key.clone(), peer.id.clone() ) }

    pub fn my_signer(&self) -> &Signer { &*self.my_signer }
    pub fn peer_pubkey(&self) -> &PublicKey { &self.peer_pubkey }
    pub fn peer_id(&self) -> &ProfileId { &self.peer_id }

    pub fn validate(&self, validator: &Validator) -> Result<(), Error>
    {
        validator.validate_profile( self.peer_pubkey(), self.peer_id() )
            .and_then( |valid|
                if valid { Ok( () ) }
                else { Err( ErrorKind::ProfileValidationFailed )? } )
    }
}



impl RelationSignablePart
{
    fn new(relation_type: &str, signer_id: &ProfileId, peer_id: &ProfileId) -> Self
        { Self{ relation_type: relation_type.to_owned(),
                signer_id: signer_id.to_owned(), peer_id: peer_id.to_owned() } }

    fn serialized(&self) -> Vec<u8> {
        // TODO unwrap() can fail here in some special cases: when there is a limit set and it's exceeded - or when .len() is
        //      not supported for the types to be serialized. Neither is possible here, so the unwrap will not fail.
        //      But still, to be on the safe side, this serialization shoule be swapped later with a call that cannot fail.
        // TODO consider using unwrap_or( Vec::new() ) instead
        serialize(self).unwrap()
    }

    fn sign(&self, signer: &Signer) -> Signature
        { signer.sign( &self.serialized() ) }
}


impl<'a> From<&'a RelationHalfProof> for RelationSignablePart {
    fn from(src: &'a RelationHalfProof) -> Self {
        RelationSignablePart{
            relation_type: src.relation_type.clone(),
            signer_id: src.signer_id.clone(),
            peer_id: src.peer_id.clone(),
        }
    }
}



impl RelationProof
{
    pub const RELATION_TYPE_HOSTED_ON_HOME:         &'static str = "hosted_on_home";
    pub const RELATION_TYPE_ENABLE_CALLS_BETWEEN:   &'static str = "enable_call_between";

    pub fn new(relation_type: &str,
               a_id: &ProfileId, a_signature: &Signature,
               b_id: &ProfileId, b_signature: &Signature) -> Self
    {
        if a_id < b_id
            { Self{ relation_type: relation_type.to_owned(),
                    a_id: a_id.to_owned(), a_signature: a_signature.to_owned(),
                    b_id: b_id.to_owned(), b_signature: b_signature.to_owned() } }
        // TODO decide on inverting relation_type if needed, e.g. `a_is_home_of_b` vs `b_is_home_of_a`
        else{ Self{ relation_type: relation_type.to_owned(),
                    a_id: b_id.to_owned(), a_signature: b_signature.to_owned(),
                    b_id: a_id.to_owned(), b_signature: a_signature.to_owned() } }
    }

    pub fn sign_remaining_half(half_proof: &RelationHalfProof, signer: &Signer) -> Result<Self, Error>
    {
        let my_profile_id = signer.profile_id().to_owned();
        if half_proof.peer_id != my_profile_id
            { Err(ErrorKind::RelationSigningFailed)? }

        let signable = RelationSignablePart::new(
            &half_proof.relation_type, &my_profile_id, &half_proof.signer_id);
        Ok( Self::new( &half_proof.relation_type, &half_proof.signer_id, &half_proof.signature,
                       &my_profile_id, &signable.sign(signer) ) )
    }

    // TODO relation-type should be more sophisticated once we have a proper metainfo schema there
    pub fn accessible_by(&self, app: &ApplicationId) -> bool
        { self.relation_type == app.0 }

    pub fn peer_id(&self, my_id: &ProfileId) -> Result<&ProfileId, Error>
    {
        if self.a_id == *my_id { return Ok(&self.b_id) }
        if self.b_id == *my_id { return Ok(&self.a_id) }
        Err(ErrorKind::PeerIdRetreivalFailed)?
    }

    pub fn peer_signature(&self, my_id: &ProfileId) -> Result<&Signature, Error>
    {
        if self.a_id == *my_id { return Ok(&self.b_signature) }
        if self.b_id == *my_id { return Ok(&self.a_signature) }
        Err(ErrorKind::PeerIdRetreivalFailed)?
    }
}



pub trait Validator: ProfileValidator + SignatureValidator
{
    fn validate_half_proof(&self, half_proof: &RelationHalfProof, signer_pubkey: &PublicKey) -> Result<(), Error> {
        self.validate_signature(signer_pubkey,
            &RelationSignablePart::from(half_proof).serialized(), &half_proof.signature)?;
        Ok(())
    }

    fn validate_relation_proof(
        &self,
        relation_proof: &RelationProof,
        id_1: &ProfileId,
        public_key_1: &PublicKey,
        id_2: &ProfileId,
        public_key_2: &PublicKey
    ) -> Result<(), Error> {
        // TODO consider inverting relation_type for different directions
        let signable_a = RelationSignablePart::new(
            &relation_proof.relation_type,
            &relation_proof.a_id,
            &relation_proof.b_id,
        ).serialized();

        let signable_b = RelationSignablePart::new(
            &relation_proof.relation_type,
            &relation_proof.b_id,
            &relation_proof.a_id,
        ).serialized();

        let peer_of_id_1 = relation_proof.peer_id(&id_1)?;
        if peer_of_id_1 != id_2 {
            Err(ErrorKind::RelationValidationFailed)?
        }

        if *peer_of_id_1 == relation_proof.b_id {
            // id_1 is 'proof.id_a'
            self.validate_signature(&public_key_1, &signable_a, &relation_proof.a_signature)?;
            self.validate_signature(&public_key_2, &signable_b, &relation_proof.b_signature)?;
        } else {
            // id_1 is 'proof.id_b'
            self.validate_signature(&public_key_1, &signable_b, &relation_proof.b_signature)?;
            self.validate_signature(&public_key_2, &signable_a, &relation_proof.a_signature)?;
        }

        Ok(())
    }
}



#[cfg(test)]
mod tests
{
    use futures::{Sink, Stream};
    use futures::sync::mpsc;
    use tokio_core::reactor;


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
    fn test_mpsc_drop_receiver()
    {
        let mut setup = TestSetup::new();
        let (sender, receiver) = mpsc::channel(2);

        // Send and item
        let item = "Hello".to_owned();
        let send_fut = sender.send( item.clone() );
        let sender = setup.reactor.run(send_fut).unwrap();

        // Receive the sent item
        // NOTE take() drops the receiver after the first element
        let recv_fut = receiver.take(1).collect();
        let recv_vec = setup.reactor.run(recv_fut).unwrap();
        assert_eq!( recv_vec.len(), 1 );
        assert_eq!( recv_vec[0], item );

        // Further sends should fail
        let send_fut = sender.send(item);
        let sender = setup.reactor.run(send_fut);
        assert!( sender.is_err() );
    }


    #[test]
    fn test_mpsc_drop_sender()
    {
        let mut setup = TestSetup::new();
        let (sender, receiver) = mpsc::channel(2);

        // Send an item and drop the sender
        let item = "Hello".to_owned();
        let send_fut = sender.send( item.clone() );
        let sender = setup.reactor.run(send_fut).unwrap();
        drop(sender);

        // Consume the stream Collecting all received elements
        let recv_fut = receiver.collect();
        let recv_vec = setup.reactor.run(recv_fut).unwrap();

        // Stream must end after dropped sender
        assert_eq!( recv_vec.len(), 1 );
        assert_eq!( recv_vec[0], item );
    }
}
