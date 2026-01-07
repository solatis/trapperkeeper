// Package server provides gRPC server lifecycle management.
package server

import (
	"context"
	"fmt"
	"net"
	"time"

	"github.com/solatis/trapperkeeper/internal/core/api"
	"github.com/solatis/trapperkeeper/internal/core/auth"
	"github.com/solatis/trapperkeeper/internal/core/config"
	pb "github.com/solatis/trapperkeeper/internal/protobuf/trapperkeeper/sensor/v1"
	"google.golang.org/grpc"
	"google.golang.org/grpc/health"
	"google.golang.org/grpc/health/grpc_health_v1"
)

// GRPCServer manages gRPC server lifecycle.
type GRPCServer struct {
	server   *grpc.Server
	listener net.Listener
	config   *config.SensorAPIConfig
}

// NewGRPCServer creates gRPC server with auth interceptor and service registration.
func NewGRPCServer(cfg *config.SensorAPIConfig, service *api.SensorAPIService, authenticator *auth.Authenticator) (*GRPCServer, error) {
	if cfg == nil {
		return nil, fmt.Errorf("cfg cannot be nil")
	}
	if service == nil {
		return nil, fmt.Errorf("service cannot be nil")
	}
	if authenticator == nil {
		return nil, fmt.Errorf("authenticator cannot be nil")
	}

	opts := []grpc.ServerOption{
		grpc.ChainUnaryInterceptor(
			authenticator.UnaryInterceptor(),
		),
	}

	server := grpc.NewServer(opts...)
	pb.RegisterSensorAPIServer(server, service)

	healthServer := health.NewServer()
	grpc_health_v1.RegisterHealthServer(server, healthServer)
	healthServer.SetServingStatus("", grpc_health_v1.HealthCheckResponse_SERVING)

	return &GRPCServer{
		server: server,
		config: cfg,
	}, nil
}

// Start binds listener and serves gRPC requests.
// Context is provided for API consistency but Serve blocks until Shutdown is called.
func (s *GRPCServer) Start(ctx context.Context) error {
	addr := fmt.Sprintf("%s:%d", s.config.Host, s.config.Port)
	listener, err := net.Listen("tcp", addr)
	if err != nil {
		return fmt.Errorf("failed to bind %s: %w", addr, err)
	}

	s.listener = listener
	return s.server.Serve(listener)
}

// Shutdown gracefully stops server with 30-second timeout.
func (s *GRPCServer) Shutdown(ctx context.Context) error {
	stopped := make(chan struct{})
	go func() {
		s.server.GracefulStop()
		close(stopped)
	}()

	select {
	case <-stopped:
		return nil
	case <-ctx.Done():
		s.server.Stop()
		return fmt.Errorf("shutdown cancelled by context: %w", ctx.Err())
	case <-time.After(30 * time.Second):
		s.server.Stop()
		return fmt.Errorf("graceful shutdown timeout, forced stop")
	}
}
