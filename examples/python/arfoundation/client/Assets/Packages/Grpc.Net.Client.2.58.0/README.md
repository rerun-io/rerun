# Grpc.Net.Client

`Grpc.Net.Client` is a gRPC client library for .NET.

## Configure gRPC client

gRPC clients are concrete client types that are [generated from `.proto` files](https://docs.microsoft.com/aspnet/core/grpc/basics#generated-c-assets). The concrete gRPC client has methods that translate to the gRPC service in the `.proto` file. For example, a service called `Greeter` generates a `GreeterClient` type with methods to call the service.

A gRPC client is created from a channel. Start by using `GrpcChannel.ForAddress` to create a channel, and then use the channel to create a gRPC client:

```csharp
var channel = GrpcChannel.ForAddress("https://localhost:5001");
var client = new Greet.GreeterClient(channel);
```

A channel represents a long-lived connection to a gRPC service. When a channel is created, it's configured with options related to calling a service. For example, the `HttpClient` used to make calls, the maximum send and receive message size, and logging can be specified on `GrpcChannelOptions` and used with `GrpcChannel.ForAddress`. For a complete list of options, see [client configuration options](https://docs.microsoft.com/aspnet/core/grpc/configuration#configure-client-options).

```csharp
var channel = GrpcChannel.ForAddress("https://localhost:5001");

var greeterClient = new Greet.GreeterClient(channel);
var counterClient = new Count.CounterClient(channel);

// Use clients to call gRPC services
```

## Make gRPC calls

A gRPC call is initiated by calling a method on the client. The gRPC client will handle message serialization and addressing the gRPC call to the correct service.

gRPC has different types of methods. How the client is used to make a gRPC call depends on the type of method called. The gRPC method types are:

* Unary
* Server streaming
* Client streaming
* Bi-directional streaming

### Unary call

A unary call starts with the client sending a request message. A response message is returned when the service finishes.

```csharp
var client = new Greet.GreeterClient(channel);
var response = await client.SayHelloAsync(new HelloRequest { Name = "World" });

Console.WriteLine("Greeting: " + response.Message);
// Greeting: Hello World
```

Each unary service method in the `.proto` file will result in two .NET methods on the concrete gRPC client type for calling the method: an asynchronous method and a blocking method. For example, on `GreeterClient` there are two ways of calling `SayHello`:

* `GreeterClient.SayHelloAsync` - calls `Greeter.SayHello` service asynchronously. Can be awaited.
* `GreeterClient.SayHello` - calls `Greeter.SayHello` service and blocks until complete. Don't use in asynchronous code.

### Server streaming call

A server streaming call starts with the client sending a request message. `ResponseStream.MoveNext()` reads messages streamed from the service. The server streaming call is complete when `ResponseStream.MoveNext()` returns `false`.

```csharp
var client = new Greet.GreeterClient(channel);
using var call = client.SayHellos(new HelloRequest { Name = "World" });

while (await call.ResponseStream.MoveNext())
{
    Console.WriteLine("Greeting: " + call.ResponseStream.Current.Message);
    // "Greeting: Hello World" is written multiple times
}
```

When using C# 8 or later, the `await foreach` syntax can be used to read messages. The `IAsyncStreamReader<T>.ReadAllAsync()` extension method reads all messages from the response stream:

```csharp
var client = new Greet.GreeterClient(channel);
using var call = client.SayHellos(new HelloRequest { Name = "World" });

await foreach (var response in call.ResponseStream.ReadAllAsync())
{
    Console.WriteLine("Greeting: " + response.Message);
    // "Greeting: Hello World" is written multiple times
}
```

### Client streaming call

A client streaming call starts *without* the client sending a message. The client can choose to send messages with `RequestStream.WriteAsync`. When the client has finished sending messages, `RequestStream.CompleteAsync()` should be called to notify the service. The call is finished when the service returns a response message.

```csharp
var client = new Counter.CounterClient(channel);
using var call = client.AccumulateCount();

for (var i = 0; i < 3; i++)
{
    await call.RequestStream.WriteAsync(new CounterRequest { Count = 1 });
}
await call.RequestStream.CompleteAsync();

var response = await call;
Console.WriteLine($"Count: {response.Count}");
// Count: 3
```

### Bi-directional streaming call

A bi-directional streaming call starts *without* the client sending a message. The client can choose to send messages with `RequestStream.WriteAsync`. Messages streamed from the service are accessible with `ResponseStream.MoveNext()` or `ResponseStream.ReadAllAsync()`. The bi-directional streaming call is complete when the `ResponseStream` has no more messages.

```csharp
var client = new Echo.EchoClient(channel);
using var call = client.Echo();

Console.WriteLine("Starting background task to receive messages");
var readTask = Task.Run(async () =>
{
    await foreach (var response in call.ResponseStream.ReadAllAsync())
    {
        Console.WriteLine(response.Message);
        // Echo messages sent to the service
    }
});

Console.WriteLine("Starting to send messages");
Console.WriteLine("Type a message to echo then press enter.");
while (true)
{
    var result = Console.ReadLine();
    if (string.IsNullOrEmpty(result))
    {
        break;
    }

    await call.RequestStream.WriteAsync(new EchoMessage { Message = result });
}

Console.WriteLine("Disconnecting");
await call.RequestStream.CompleteAsync();
await readTask;
```

For best performance, and to avoid unnecessary errors in the client and service, try to complete bi-directional streaming calls gracefully. A bi-directional call completes gracefully when the server has finished reading the request stream and the client has finished reading the response stream. The preceding sample call is one example of a bi-directional call that ends gracefully. In the call, the client:

1. Starts a new bi-directional streaming call by calling `EchoClient.Echo`.
2. Creates a background task to read messages from the service using `ResponseStream.ReadAllAsync()`.
3. Sends messages to the server with `RequestStream.WriteAsync`.
4. Notifies the server it has finished sending messages with `RequestStream.CompleteAsync()`.
5. Waits until the background task has read all incoming messages.

During a bi-directional streaming call, the client and service can send messages to each other at any time. The best client logic for interacting with a bi-directional call varies depending upon the service logic.

## Links

* [Documentation](https://docs.microsoft.com/aspnet/core/grpc/client)
* [grpc-dotnet GitHub](https://github.com/grpc/grpc-dotnet)
