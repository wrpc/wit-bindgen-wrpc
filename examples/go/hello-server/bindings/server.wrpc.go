// Generated by `wit-bindgen-wrpc-go` 0.1.1. DO NOT EDIT!
// server package contains wRPC bindings for `server` world
package server

import (
	exports__wrpc_examples__hello__handler "github.com/wrpc/wit-bindgen-wrpc/examples/go/hello-server/bindings/exports/wrpc_examples/hello/handler"
	wrpc "github.com/wrpc/wrpc/go"
)

func Serve(s wrpc.Server, h0 exports__wrpc_examples__hello__handler.Handler) (stop func() error, err error) {
	stops := make([]func() error, 0, 1)
	stop = func() error {
		for _, stop := range stops {
			if err := stop(); err != nil {
				return err
			}
		}
		return nil
	}
	stop0, err := exports__wrpc_examples__hello__handler.ServeInterface(s, h0)
	if err != nil {
		return
	}
	stops = append(stops, stop0)
	stop = func() error {
		if err := stop0(); err != nil {
			return err
		}
		return nil
	}
	return
}
