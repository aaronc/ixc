use crate::handler::PublishedFnInfo;
use crate::util::push_item;
use core::borrow::Borrow;
use heck::ToUpperCamelCase;
use manyhow::{bail, ensure};
use proc_macro2::{Ident, Literal, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::punctuated::Punctuated;
use syn::{parse_quote, Item, ReturnType, Signature, Type};

#[derive(Default)]
pub(crate) struct APIBuilder {
    pub(crate) items: Vec<Item>,
    pub(crate) routes: Vec<TokenStream2>,
    pub(crate) query_routes: Vec<TokenStream2>,
    pub(crate) system_routes: Vec<TokenStream2>,
    pub(crate) client_signatures: Vec<Signature>,
    client_methods: Vec<TokenStream2>,
    pub(crate) create_msg_name: Option<Ident>,
    pub(crate) create_msg_lifetime: TokenStream2,
}

impl APIBuilder {
    /// Defines the client struct and makes it implement the Client trait.
    pub(crate) fn define_client(&mut self, client_ident: &Ident) -> manyhow::Result<()> {
        push_item(
            &mut self.items,
            quote! {
                pub struct #client_ident(::ixc::message_api::AccountID);
            },
        )?;
        push_item(
            &mut self.items,
            quote! {
                impl ::ixc::core::handler::Client for #client_ident {
                    fn new(account_id: ::ixc::message_api::AccountID) -> Self {
                        Self(account_id)
                    }

                    fn account_id(&self) -> ::ixc::message_api::AccountID {
                        self.0
                    }
                }
            },
        )
    }

    /// Defines the client struct's implementation.
    pub(crate) fn define_client_impl(
        &mut self,
        impl_target: &TokenStream2,
        visibility: &TokenStream2,
    ) -> manyhow::Result<()> {
        let client_methods = &self.client_methods;
        push_item(
            &mut self.items,
            quote! {
                impl #impl_target {
                    #(#visibility #client_methods)*
                }
            },
        )
    }

    /// Implement the service trait for the client struct.
    pub(crate) fn define_client_service(
        &mut self,
        client_ident: &Ident,
        factory_target: &TokenStream2,
    ) -> manyhow::Result<()> {
        push_item(
            &mut self.items,
            quote! {
                impl ::ixc::core::handler::Service for #factory_target {
                    type Client = #client_ident;
                }
            },
        )
    }

    // This is probably the function that does most of the work in the whole macro system.
    // It extracts the data from the function and generates:
    // - a message struct
    // - the code for calling it as a message handler
    // - a client version of the function
    pub(crate) fn extract_method_data(
        &mut self,
        handler_ident: &Ident,
        _handler_ty: &TokenStream2,
        publish_target: &PublishedFnInfo,
    ) -> manyhow::Result<()> {
        let signature = &publish_target.signature;
        let fn_name = &signature.ident;
        // we generate the message struct name by appending the camel case version of the function name to the handler name
        let fn_name_camel = fn_name.to_string().to_upper_camel_case();
        let msg_struct_name = format_ident!("{}{}", handler_ident, fn_name_camel);
        let mut signature = signature.clone();
        // we use this to collect the client fn arguments which may differ slightly from the original fn arguments
        let mut client_inputs = Punctuated::new();
        // these are the fields of the message struct
        let mut msg_fields = vec![];
        // this is used for destructuring the message struct fields
        let mut msg_deconstruct = vec![];
        // these are the arguments to the function after being decoded from the message struct
        let mut fn_call_args = vec![];
        // these are the field initializers for the message struct when it is being created
        let mut msg_fields_init = vec![];
        // whether or not the message struct has lifetimes
        let mut msg_has_lifetimes = false;
        // the name of the context parameter if it exists
        let mut context_name: Option<Ident> = None;
        // whether or not the function is a query, meaning it has &Context rather than &mut Context
        let mut is_query = false;
        for field in &signature.inputs {
            match field {
                // check that we have a &self receiver
                syn::FnArg::Receiver(r) => {
                    if r.mutability.is_some() {
                        bail!(
                            "error with fn {}: &self receiver on published fn's must be immutable",
                            fn_name
                        );
                    }
                }
                // other function inputs should end up here
                syn::FnArg::Typed(pat_type) => {
                    match pat_type.pat.as_ref() {
                        syn::Pat::Ident(ident) => {
                            let mut ty = pat_type.ty.clone();
                            match ty.as_mut() {
                                // reference types end up in this case
                                Type::Reference(tyref) => {
                                    // here the main check is whether we have Context or an EventBus
                                    if let Type::Path(path) = tyref.elem.borrow() {
                                        if path.path.segments.first().unwrap().ident == "Context" {
                                            context_name = Some(ident.ident.clone());
                                            client_inputs.push(field.clone()); // we add the input parameter to the client function because we're going to call continue
                                            if tyref.mutability.is_none() {
                                                is_query = true;
                                            }
                                            // we continue because we don't want to add Context to the message struct
                                            continue;
                                        }

                                        if let Some(s) = path.path.segments.first() {
                                            if s.ident == "EventBus" {
                                                fn_call_args
                                                    .push(quote! { &mut Default::default(), });
                                                // we continue because we don't want to add the EventBus to the message struct or the client function
                                                continue;
                                            }
                                        }
                                    }

                                    // otherwise we have some input parameter that is borrowed
                                    // and we just need to make sure that it's lifetime is called 'a or unnamed
                                    msg_has_lifetimes = true;
                                    assert!(
                                        tyref.lifetime.is_none()
                                            || tyref.lifetime.as_ref().unwrap().ident == "a",
                                        "lifetime must be either unnamed or called 'a"
                                    );
                                    tyref.lifetime = Some(parse_quote!('a));
                                }
                                // value types end up here
                                Type::Path(path) => {
                                    // we also accept a non-borrowed event bus
                                    if let Some(s) = path.path.segments.first() {
                                        if s.ident == "EventBus" {
                                            fn_call_args.push(quote! { Default::default(), });
                                            // we continue because we don't want to add the EventBus to the message struct or the client function
                                            continue;
                                        }
                                    }
                                    // note that we don't need to do anything special for most value types here
                                }
                                _ => {}
                            }
                            // push this input parameter to the message struct
                            msg_fields.push(quote! {
                                pub #ident: #ty,
                            });
                            // push this input parameter to the message struct deconstructor
                            msg_deconstruct.push(quote! {
                                #ident,
                            });
                            // push this input parameter to the function call
                            fn_call_args.push(quote! {
                                #ident,
                            });
                            // push this input parameter to the message struct initializer
                            msg_fields_init.push(quote! {
                                #ident,
                            });
                        }
                        _ => bail!("expected identifier"),
                    };
                }
                _ => {}
            }
            // push any parameters where we haven't called continue yet to the client function (Context should already have been added higher up)
            client_inputs.push(field.clone());
        }
        // signature now represents the client function
        signature.inputs = client_inputs;
        // we need to add lifetimes parameters to impls if the message struct has lifetimes
        // so we use this to generate that code
        let opt_lifetime = if msg_has_lifetimes {
            quote! { <'a> }
        } else {
            quote! {}
        };
        // same thing but sometimes we don't need a named lifetime
        let opt_underscore_lifetime = if msg_has_lifetimes {
            quote! { <'_> }
        } else {
            quote! {}
        };

        // generate the message struct and push it into the mod's block's items
        push_item(
            &mut self.items,
            quote! {
                #[derive(::ixc::SchemaValue, Default)]
                #[sealed]
                pub struct #msg_struct_name #opt_lifetime {
                    #(#msg_fields)*
                }
            },
        )?;

        // calculate the message selector
        let return_type = match &signature.output {
            ReturnType::Type(_, ty) => ty,
            ReturnType::Default => {
                bail!("expected return type")
            }
        };
        let msg_struct_name_str = Literal::string(&msg_struct_name.to_string());
        if publish_target.on_create.is_none() {
            push_item(
                &mut self.items,
                quote! {
                    impl <'a> ::ixc::core::message::MessageBase<'a> for #msg_struct_name #opt_lifetime {
                        const SELECTOR: ::ixc::message_api::message_selector::MessageSelector = ::ixc::core::message_selector!(#msg_struct_name_str);
                        type Response<'b> = <#return_type as ::ixc::core::message::ExtractResponseTypes>::Response;
                        type Error = <#return_type as ::ixc::core::message::ExtractResponseTypes>::Error;
                        type Codec = ::ixc::schema::binary::NativeBinaryCodec;
                    }
                },
            )?;
            push_item(
                &mut self.items,
                if is_query {
                    quote! { impl <'a> ::ixc::core::message::QueryMessage<'a> for #msg_struct_name #opt_lifetime {} }
                } else {
                    quote! { impl <'a> ::ixc::core::message::Message<'a> for #msg_struct_name #opt_lifetime {} }
                },
            )?;
            ensure!(context_name.is_some(), "no context parameter found");
            let context_name = context_name.unwrap();
            let (maybe_mut, new_ctx) = if is_query {
                (quote! {}, quote! { new })
            } else {
                (quote! { mut }, quote! { new_mut })
            };
            let route = quote! {
                        (< #msg_struct_name #opt_underscore_lifetime as ::ixc::core::message::MessageBase >::SELECTOR, |h: &Self, packet, cb, a| {
                            unsafe {
                                let cdc = < #msg_struct_name as ::ixc::core::message::MessageBase<'_> >::Codec::default();
                                let header = packet.header();
                                let in1 = header.in_pointer1.get(packet);
                                let mem = ::ixc::schema::mem::MemoryManager::new();
                                let #msg_struct_name { #(#msg_deconstruct)* } = ::ixc::schema::codec::decode_value::< #msg_struct_name >(&cdc, in1, &mem)?;
                                let #maybe_mut ctx = ::ixc::core::Context::#new_ctx(header.account, header.caller, header.gas_left, cb, &mem);
                                let res = h.#fn_name(& #maybe_mut ctx, #(#fn_call_args)*);
                                ::ixc::core::low_level::encode_response::< #msg_struct_name >(&cdc, res, a, packet)
                            }
                        }),
            };
            if is_query {
                self.query_routes.push(route);
            } else {
                self.routes.push(route);
            }
            signature.output = parse_quote! {
                -> <#return_type as ::ixc::core::message::ExtractResponseTypes>::ClientResult
            };
            self.client_signatures.push(signature.clone());
            let dynamic_invoke = if is_query {
                quote! { ::ixc::core::low_level::dynamic_invoke_query(ctx, _acct_id, _msg) }
            } else {
                quote! { ::ixc::core::low_level::dynamic_invoke_msg(ctx, _acct_id, _msg) }
            };
            self.client_methods.push(quote! {
                    #signature {
                        let _msg = #msg_struct_name {
                            #(#msg_fields_init)*
                        };
                        let _acct_id = ::ixc::core::handler::Client::account_id(self);
                        unsafe {
                            #dynamic_invoke
                        }
                    }
            });
        } else {
            self.system_routes.push(quote! {
                (::ixc::core::account_api::ON_CREATE_SELECTOR, | h: &Self, packet, cb, a| {
                    unsafe {
                        let cdc = < #msg_struct_name #opt_underscore_lifetime as::ixc::core::handler::InitMessage<'_> >::Codec::default();
                        let header = packet.header();
                        let in1 = header.in_pointer1.get(packet);
                        let mem = ::ixc::schema::mem::MemoryManager::new();
                        let #msg_struct_name { #(#msg_deconstruct)* } = ::ixc::schema::codec::decode_value::< #msg_struct_name > ( & cdc, in1, &mem)?;
                        let mut ctx =::ixc::core::Context::new_mut(header.account, header.caller, header.gas_left, cb, &mem);
                        let res = h.#fn_name(&mut ctx, #(#fn_call_args)*);
                        ::ixc::core::low_level::encode_default_response(res, a, packet)
                    }
                }),}
            );
            self.create_msg_name = Some(msg_struct_name);
            self.create_msg_lifetime = opt_lifetime;
        }
        Ok(())
    }

    pub(crate) fn impl_router(&mut self, target: TokenStream2) -> manyhow::Result<()> {
        let routes = &self.routes;
        let query_routes = &self.query_routes;
        let system_routes = &self.system_routes;
        push_item(
            &mut self.items,
            quote! {
                unsafe impl ::ixc::core::routing::Router for #target {
                    const SORTED_MSG_ROUTES: &'static [::ixc::core::routing::Route<Self>] =
                        &::ixc::core::routing::sort_routes([
                            #(#routes)*
                        ]);

                    const SORTED_QUERY_ROUTES: &'static [::ixc::core::routing::QueryRoute<Self>] =
                        &::ixc::core::routing::sort_query_routes([
                            #(#query_routes)*
                        ]);

                    const SORTED_SYSTEM_ROUTES: &'static [::ixc::core::routing::Route<Self>] =
                        &::ixc::core::routing::sort_routes([
                            #(#system_routes)*
                        ]);
                }
            },
        )
    }
}
