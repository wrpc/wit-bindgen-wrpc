use core::str;
use core::time::Duration;

use std::sync::Arc;

use anyhow::{ensure, Context};
use futures::FutureExt as _;
use tokio::sync::{oneshot, Notify, RwLock};
use tokio::time::sleep;
use tokio::try_join;

mod common;
use common::{init, with_nats};

#[tokio::test(flavor = "multi_thread")]
async fn rust_bindgen() -> anyhow::Result<()> {
    init().await;

    with_nats(|_, nats_client| async {
        let client = wrpc_transport_nats::Client::new(nats_client, "test-prefix".to_string());
        let client = Arc::new(client);

        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let shutdown_rx =
            async move { shutdown_rx.await.expect("shutdown sender dropped") }.shared();
        try_join!(
            async {
                wit_bindgen_wrpc::generate!({
                    inline: "
                        package wrpc-test:integration;

                        interface shared {
                            flags abc {
                                a,
                                b,
                                c,
                            }

                            fallible: func() -> result<bool, string>;
                            numbers: func() -> tuple<u8, u16, u32, u64, s8, s16, s32, s64, f32, f64>;
                            with-flags: func() -> abc;
                        }

                        world test {
                            export shared;

                            export f: func(x: string) -> u32;
                            export foo: interface {
                                foo: func(x: string);
                            }
                        }"
                });

                #[derive(Clone, Default)]
                struct Component(Arc<RwLock<Option<String>>>);

                impl Handler<Option<async_nats::HeaderMap>> for Component {
                    async fn f(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                        x: String,
                    ) -> anyhow::Result<u32> {
                        let stored = self.0.read().await.as_ref().unwrap().to_string();
                        assert_eq!(stored, x);
                        Ok(42)
                    }
                }

                impl exports::wrpc_test::integration::shared::Handler<Option<async_nats::HeaderMap>> for Component {
                    async fn fallible(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                    ) -> anyhow::Result<Result<bool, String>> {
                        Ok(Ok(true))
                    }

                    async fn numbers(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                    ) -> anyhow::Result<(u8, u16, u32, u64, i8, i16, i32, i64, f32, f64)> {
                        Ok((
                            0xfe,
                            0xfeff,
                            0xfeff_ffff,
                            0xfeff_ffff_ffff_ffff,
                            0x7e,
                            0x7eff,
                            0x7eff_ffff,
                            0x7eff_ffff_ffff_ffff,
                            0.42,
                            0.4242,
                        ))
                    }

                    async fn with_flags(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                    ) -> anyhow::Result<exports::wrpc_test::integration::shared::Abc> {
                        use exports::wrpc_test::integration::shared::Abc;
                        Ok(Abc::A | Abc::C)
                    }
                }

                impl exports::foo::Handler<Option<async_nats::HeaderMap>> for Component {
                    async fn foo(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                        x: String,
                    ) -> anyhow::Result<()> {
                        let old = self.0.write().await.replace(x);
                        assert!(old.is_none());
                        Ok(())
                    }
                }

                serve(client.as_ref(), Component::default(), shutdown_rx.clone())
                    .await
                    .context("failed to serve `wrpc-test:integration/test`")
            },
            async {
                wit_bindgen_wrpc::generate!({
                    inline: "
                        package wrpc-test:integration;

                        interface shared {
                            flags abc {
                                a,
                                b,
                                c,
                            }

                            fallible: func() -> result<bool, string>;
                            numbers: func() -> tuple<u8, u16, u32, u64, s8, s16, s32, s64, f32, f64>;
                            with-flags: func() -> abc;
                        }

                        world test {
                            import shared;

                            import f: func(x: string) -> u32;
                            import foo: interface {
                                foo: func(x: string);
                            }
                            export bar: interface {
                                bar: func() -> string;
                            }
                        }"
                });

                #[derive(Clone)]
                struct Component(Arc<wrpc_transport_nats::Client>);

                // TODO: Remove the need for this
                sleep(Duration::from_secs(1)).await;

                impl exports::bar::Handler<Option<async_nats::HeaderMap>> for Component {
                    async fn bar(
                        &self,
                        _cx: Option<async_nats::HeaderMap>,
                    ) -> anyhow::Result<String> {
                        use wrpc_test::integration::shared::Abc;

                        foo::foo(self.0.as_ref(), "foo")
                            .await
                            .context("failed to call `wrpc-test:integration/test.foo.foo`")?;
                        let v = f(self.0.as_ref(), "foo")
                            .await
                            .context("failed to call `wrpc-test:integration/test.f`")?;
                        assert_eq!(v, 42);
                        let v = wrpc_test::integration::shared::fallible(self.0.as_ref())
                            .await
                            .context("failed to call `wrpc-test:integration/shared.fallible`")?;
                        assert_eq!(v, Ok(true));
                        let v = wrpc_test::integration::shared::numbers(self.0.as_ref())
                            .await
                            .context("failed to call `wrpc-test:integration/shared.numbers`")?;
                        assert_eq!(v, (
                            0xfe,
                            0xfeff,
                            0xfeff_ffff,
                            0xfeff_ffff_ffff_ffff,
                            0x7e,
                            0x7eff,
                            0x7eff_ffff,
                            0x7eff_ffff_ffff_ffff,
                            0.42,
                            0.4242,
                        ));
                        let v = wrpc_test::integration::shared::with_flags(self.0.as_ref())
                            .await
                            .context("failed to call `wrpc-test:integration/shared.with-flags`")?;
                        assert_eq!(v, Abc::A | Abc::C);
                        Ok("bar".to_string())
                    }
                }

                serve(
                    client.as_ref(),
                    Component(Arc::clone(&client)),
                    shutdown_rx.clone(),
                )
                .await
                .context("failed to serve `wrpc-test:integration/test`")
            },
            async {
                wit_bindgen_wrpc::generate!({
                    inline: "
                        package wrpc-test:integration;

                        world test {
                            import bar: interface {
                                bar: func() -> string;
                            }
                        }"
                });

                // TODO: Remove the need for this
                sleep(Duration::from_secs(2)).await;

                let v = bar::bar(client.as_ref())
                    .await
                    .context("failed to call `wrpc-test:integration/test.bar.bar`")?;
                assert_eq!(v, "bar");
                shutdown_tx.send(()).expect("failed to send shutdown");
                Ok(())
            },
        )?;
        Ok(())
    }).await
}

#[tokio::test(flavor = "multi_thread")]
async fn rust_keyvalue() -> anyhow::Result<()> {
    init().await;

    with_nats(|_, nats_client| async {
        let client = wrpc_transport_nats::Client::new(nats_client, "test-prefix".to_string());

        let shutdown = Notify::new();
        let started = Notify::new();

        try_join!(
            async {
                mod bindings {
                    wit_bindgen_wrpc::generate!("keyvalue-server");
                }

                #[derive(Clone)]
                struct Handler;
                use bindings::exports::wrpc::keyvalue;
                type Result<T, E = keyvalue::store::Error> = core::result::Result<T, E>;

                impl<Ctx: Send> keyvalue::store::Handler<Ctx> for Handler {
                    async fn delete(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        key: String,
                    ) -> anyhow::Result<Result<()>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(key, "key");
                        Ok(Ok(()))
                    }

                    async fn exists(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        key: String,
                    ) -> anyhow::Result<Result<bool>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(key, "key");
                        Ok(Ok(true))
                    }

                    async fn get(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        key: String,
                    ) -> anyhow::Result<Result<Option<Vec<u8>>>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(key, "key");
                        Ok(Ok(Some(vec![0x42, 0xff])))
                    }

                    async fn set(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        key: String,
                        value: Vec<u8>,
                    ) -> anyhow::Result<Result<()>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(key, "key");
                        assert_eq!(value, b"test");
                        Ok(Ok(()))
                    }

                    async fn list_keys(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        cursor: Option<u64>,
                    ) -> anyhow::Result<Result<keyvalue::store::KeyResponse>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(cursor, Some(42));
                        Ok(Ok(keyvalue::store::KeyResponse {
                            cursor: None,
                            keys: vec!["key".to_string()],
                        }))
                    }
                }

                impl<Ctx: Send> keyvalue::atomics::Handler<Ctx> for Handler {
                    async fn increment(
                        &self,
                        _cx: Ctx,
                        bucket: String,
                        key: String,
                        delta: u64,
                    ) -> anyhow::Result<Result<u64, keyvalue::store::Error>> {
                        assert_eq!(bucket, "bucket");
                        assert_eq!(key, "key");
                        assert_eq!(delta, 42);
                        Ok(Ok(4242))
                    }
                }

                let fut = bindings::serve(&client, Handler, shutdown.notified());

                started.notify_one();

                fut.await.context("failed to serve world")
            },
            async {
                mod bindings {
                    wit_bindgen_wrpc::generate!({
                        world: "keyvalue-client",
                        additional_derives: [Eq, PartialEq],
                    });
                }
                use bindings::wrpc::keyvalue;

                started.notified().await;

                try_join!(
                    async {
                        keyvalue::store::delete(&client, "bucket", "key")
                            .await
                            .context("failed to call `delete`")?
                            .context("`delete` call failed")
                    },
                    async {
                        let v = keyvalue::store::exists(&client, "bucket", "key")
                            .await
                            .context("failed to call `exists`")?
                            .context("`exists` call failed")?;
                        ensure!(v, "`exists` should have returned `true`");
                        Ok(())
                    },
                    async {
                        let v = keyvalue::store::get(&client, "bucket", "key")
                            .await
                            .context("failed to call `get`")?
                            .context("`get` call failed")?;
                        ensure!(
                            v.as_deref() == Some(&[0x42, 0xff]),
                            "`get` should have returned `Some([0x42, 0xff])`, got `{v:?}`"
                        );
                        Ok(())
                    },
                    async {
                        keyvalue::store::set(&client, "bucket", "key", b"test")
                            .await
                            .context("failed to call `set`")?
                            .context("`set` call failed")
                    },
                    async {
                        let v = keyvalue::store::list_keys(&client, "bucket", Some(42))
                            .await
                            .context("failed to call `list-keys`")?
                            .context("`list-keys` call failed")?;
                        ensure!(
                            v == keyvalue::store::KeyResponse {
                                cursor: None,
                                keys: vec!["key".to_string()]
                            },
                            r#"`list-keys` should have returned `{{None, ["key"]}}`, got `{v:?}`"#
                        );
                        Ok(())
                    },
                )?;
                shutdown.notify_one();
                Ok(())
            }
        )?;
        Ok(())
    })
    .await
}
