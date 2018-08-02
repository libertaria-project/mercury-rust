use super::*;
use mercury_connect::sdk::*;
use mercury_home_protocol::*;
use futures::IntoFuture;
use futures::Stream;
use std::rc::Rc;

pub struct Client {
    appctx : AppContext,
    cfg: ClientConfig,
    mercury_app: Box<DAppInit>,
}

impl Client{
    pub fn new(cfg: ClientConfig, appctx: AppContext) -> Self{
        unimplemented!();
        /*
        Self{
            appctx: appctx,
            cfg: cfg,
            fut: None,
            // mercury: Box::new(DappConnect::new(appctx.priv_key))
        }
        */
    }
}

impl IntoFuture for Client {
    type Item = ();
    type Error = std::io::Error;
    type Future = Box<Future<Item=Self::Item, Error=Self::Error>>;

    fn into_future(self) -> Self::Future {
        let callee_profile_id = self.cfg.callee_profile_id.clone();

        let f = self.mercury_app.initialize(&ApplicationId("the button".to_string()))
                .and_then(move |api: Rc<DAppApi>| {
                    info!("application initialized, calling {:?}", callee_profile_id);
                    api.call(&self.cfg.callee_profile_id, AppMessageFrame(vec![]))                    
                })
                .map_err(|err| {
                    error!("call failed: {:?}", err);
                    ()
                })
                .and_then(|call: Call| {
                    info!("call accepted, waiting for incoming messages");
                    call.receiver
                        .for_each(|msg: Result<AppMessageFrame, String>| {
                            match msg {
                                Ok(frame) => {
                                    info!("got message {:?}", frame); 
                                    Ok(())
                                },
                                Err(errmsg) => {
                                    warn!("got error {:?}", errmsg); 
                                    Err(())
                                }
                            }
                        })                        
                })
                .map_err(|_err| std::io::Error::new(std::io::ErrorKind::Other, "encountered error"));
        Box::new(f)
    }
}

