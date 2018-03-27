use capnp::capability::Promise;
use futures::{Future};
use futures::future;
use multiaddr::{Multiaddr, AddrComponent};
use tokio_core::reactor;
use tokio_core::net::TcpStream;
use tokio_io::AsyncRead;

use mercury_common::mercury_capnp;
use mercury_common::mercury_capnp::*;

use super::*;



pub fn capnp_home(tcp_stream: TcpStream, context: Box<PeerContext>, handle: reactor::Handle) -> Rc<Home>
{
    Rc::new( HomeClientCapnProto::new(tcp_stream, context, handle) )
}



pub struct HomeClientCapnProto
{
    context:Box<PeerContext>,
    repo:   mercury_capnp::profile_repo::Client<>,
    home:   mercury_capnp::home::Client<>,
}


impl HomeClientCapnProto
{
    pub fn new(tcp_stream: TcpStream, context: Box<PeerContext>,
               handle: reactor::Handle) -> Self
    {
        println!("Initializing Cap'n'Proto");
        tcp_stream.set_nodelay(true).unwrap();
        let (reader, writer) = tcp_stream.split();

        // TODO maybe we should set up only single party capnp first
        let rpc_network = Box::new( capnp_rpc::twoparty::VatNetwork::new( reader, writer,
                                                                          capnp_rpc::rpc_twoparty_capnp::Side::Client, Default::default() ) );
        let mut rpc_system = capnp_rpc::RpcSystem::new(rpc_network, None);

        let home: mercury_capnp::home::Client =
            rpc_system.bootstrap(capnp_rpc::rpc_twoparty_capnp::Side::Server);
        let repo: mercury_capnp::profile_repo::Client =
            rpc_system.bootstrap(capnp_rpc::rpc_twoparty_capnp::Side::Server);

        handle.spawn( rpc_system.map_err( |e| println!("Capnp RPC failed: {}", e) ) );

        Self{ context: context, home: home, repo: repo } // , rpc_system: rpc_system
    }
}



// TODO is this needed here or elsewhere?
//impl PeerContext for HomeClientCapnProto
//{
//    fn my_signer(&self)     -> &Signer          { self.context.my_signer() }
//    fn peer_pubkey(&self)   -> Option<PublicKey>{ self.context.peer_pubkey() }
//    fn peer(&self)          -> Option<Profile>  { self.context.peer() }
//}



impl ProfileRepo for HomeClientCapnProto
{
    fn list(&self, /* TODO what filter criteria should we have here? */ ) ->
    Box< Stream<Item=Profile, Error=ErrorToBeSpecified> >
    {
        // TODO properly implement this
        let (_send, recv) = futures::sync::mpsc::channel(0);
        Box::new( recv.map_err( |_| ErrorToBeSpecified::TODO ) )
    }


    fn load(&self, id: &ProfileId) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >
    {
        let mut request = self.repo.load_request();
        request.get().set_profile_id( id.0.as_slice() );

        let resp_fut = request.send().promise
            .and_then( |resp|
                {
                    let profile_capnp = pry!( pry!( resp.get() ).get_profile() );
                    let profile = Profile::try_from(profile_capnp);
                    Promise::result(profile)
                } )
            .map_err( |e| { println!("checkin() failed {}", e); ErrorToBeSpecified::TODO } );

        Box::new(resp_fut)
    }

    // NOTE should be more efficient than load(id) because URL is supposed to contain hints for resolution
    fn resolve(&self, url: &str) ->
    Box< Future<Item=Profile, Error=ErrorToBeSpecified> >
    {
        let mut request = self.repo.resolve_request();
        request.get().set_profile_url(url);

        let resp_fut = request.send().promise
            .and_then( |resp|
                {
                    let profile_capnp = pry!( pry!( resp.get() ).get_profile() );
                    let profile = Profile::try_from(profile_capnp);
                    Promise::result(profile)
                } )
            .map_err( |e| { println!("checkin() failed {}", e); ErrorToBeSpecified::TODO } );

        Box::new(resp_fut)
    }
}



impl Home for HomeClientCapnProto
{
    fn claim(&self, profile_id: ProfileId) ->
    Box< Future<Item=OwnProfile, Error=ErrorToBeSpecified> >
    {
        let mut request = self.home.claim_request();
        request.get().set_profile_id( (&profile_id).into() );

        let resp_fut = request.send().promise
            .and_then( |resp|
                resp.get()
                    .and_then( |res| res.get_own_profile() )
                    .and_then( |own_prof_capnp| OwnProfile::try_from(own_prof_capnp) ) )
            .map_err( |e| { println!("login() failed {}", e); ErrorToBeSpecified::TODO } );;

        Box::new(resp_fut)
    }

    fn register(&self, own_profile: OwnProfile, invite: Option<HomeInvitation>) ->
    Box< Future<Item=OwnProfile, Error=(OwnProfile,ErrorToBeSpecified)> >
    {
        let mut request = self.home.register_request();
        request.get().init_own_profile().fill_from(&own_profile);
        if let Some(inv) = invite
            { request.get().init_invite().fill_from(&inv); }

        let resp_fut = request.send().promise
            .and_then( |resp|
                resp.get()
                    .and_then( |res| res.get_own_profile() )
                    .and_then( |own_prof_capnp| OwnProfile::try_from(own_prof_capnp) ) )
            .map_err( move |e| (own_profile, ErrorToBeSpecified::TODO) );;

        Box::new(resp_fut)
    }


    fn login(&self, profile_id: ProfileId) ->
    Box< Future<Item=Box<HomeSession>, Error=ErrorToBeSpecified> >
    {
        let mut request = self.home.login_request();
        request.get().set_profile_id( (&profile_id).into() );

        let resp_fut = request.send().promise
            .and_then( |resp|
                {
                    resp.get()
                        .and_then( |res| res.get_session() )
                        .map( |session_client| Box::new( HomeSessionClientCapnProto::new(session_client) ) as Box<HomeSession> )
                } )
            .map_err( |e| { println!("login() failed {}", e); ErrorToBeSpecified::TODO } );;

        Box::new(resp_fut)
    }


    // NOTE acceptor must have this server as its home
    fn pair_request(&self, half_proof: RelationHalfProof) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        Box::new( futures::future::err(ErrorToBeSpecified::TODO) )
    }

    // NOTE acceptor must have this server as its home
    fn pair_response(&self, rel: RelationProof) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        Box::new( futures::future::err(ErrorToBeSpecified::TODO) )
    }

    fn call(&self, rel: RelationProof, app: ApplicationId, init_payload: AppMessageFrame) ->
    Box< Future<Item=CallMessages, Error=ErrorToBeSpecified> >
    {
        Box::new( futures::future::err(ErrorToBeSpecified::TODO) )
    }
}



pub struct HomeSessionClientCapnProto
{
    session: mercury_capnp::home_session::Client<>,
}

impl HomeSessionClientCapnProto
{
    pub fn new(session: mercury_capnp::home_session::Client) -> Self
    { Self{ session: session } }
}

impl HomeSession for HomeSessionClientCapnProto
{
    // TODO consider if we should notify an open session about an updated profile
    fn update(&self, own_prof: &OwnProfile) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        Box::new( futures::future::err(ErrorToBeSpecified::TODO) )
    }

    // NOTE newhome is a profile that contains at least one HomeFacet different than this home
    fn unregister(&self, newhome: Option<Profile>) ->
    Box< Future<Item=(), Error=ErrorToBeSpecified> >
    {
        Box::new( futures::future::err(ErrorToBeSpecified::TODO) )
    }


    fn events(&self) -> Rc< Stream<Item=ProfileEvent, Error=ErrorToBeSpecified> >
    {
        let (_send, recv) = futures::sync::mpsc::channel(0);
        Rc::new( recv.map_err( |_| ErrorToBeSpecified::TODO ) )
    }

    // TODO return not a Stream, but an AppSession struct containing a stream
    fn checkin_app(&self, app: &ApplicationId) ->
    Box< Stream<Item=Call, Error=ErrorToBeSpecified> >
    {
        let (_send, recv) = futures::sync::mpsc::channel(0);
        Box::new( recv.map_err( |_| ErrorToBeSpecified::TODO ) )
    }

    fn ping(&self, txt: &str) ->
    Box< Future<Item=String, Error=ErrorToBeSpecified> >
    {
        println!("checkin() called");
        let mut request = self.session.ping_request();
        request.get().set_txt(txt);
        println!("checkin request created");

        let resp_fut = request.send().promise
            .and_then( |resp|
                {
                    println!("checkin() message sent");
                    resp.get()
                        .and_then( |res|
                            res.get_pong().map( |s| s.to_owned() ) )
                } )
            .map_err( |e| { println!("checkin() failed {}", e); ErrorToBeSpecified::TODO } );

        Box::new(resp_fut)
    }
}