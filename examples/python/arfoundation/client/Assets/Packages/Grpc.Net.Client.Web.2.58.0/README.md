# Grpc.Net.Client.Web

The .NET gRPC client can be configured to make gRPC-Web calls. This is useful for [Blazor WebAssembly](https://docs.microsoft.com/aspnet/core/blazor#blazor-webassembly) apps, which are hosted in the browser and have the same HTTP limitations of JavaScript code. Calling gRPC-Web with a .NET client is [the same as HTTP/2 gRPC](https://docs.microsoft.com/aspnet/core/grpc/client). The only modification is how the channel is created.

To use gRPC-Web:

* Add a reference to the [Grpc.Net.Client.Web](https://www.nuget.org/packages/Grpc.Net.Client.Web) package.
* Ensure the reference to [Grpc.Net.Client](https://www.nuget.org/packages/Grpc.Net.Client) package is 2.29.0 or greater.
* Configure the channel to use the `GrpcWebHandler`:

```csharp
var channel = GrpcChannel.ForAddress("https://localhost:5001", new GrpcChannelOptions
    {
        HttpHandler = new GrpcWebHandler(new HttpClientHandler())
    });

var client = new Greeter.GreeterClient(channel);
var response = await client.SayHelloAsync(new HelloRequest { Name = ".NET" });
```

The preceding code:

* Configures a channel to use gRPC-Web.
* Creates a client and makes a call using the channel.

`GrpcWebHandler` has the following configuration options:

* **InnerHandler**: The underlying [`HttpMessageHandler`](https://docs.microsoft.com/dotnet/api/system.net.http.httpmessagehandler) that makes the gRPC HTTP request, for example, `HttpClientHandler`.
* **GrpcWebMode**: An enumeration type that specifies whether the gRPC HTTP request `Content-Type` is `application/grpc-web` or `application/grpc-web-text`.
    * `GrpcWebMode.GrpcWeb` configures content to be sent without encoding. Default value.
    * `GrpcWebMode.GrpcWebText` configures content to be base64 encoded. Required for server streaming calls in browsers.
* **HttpVersion**: HTTP protocol `Version` used to set [`HttpRequestMessage.Version`](https://docs.microsoft.com/dotnet/api/system.net.http.httprequestmessage.version#system-net-http-httprequestmessage-version) on the underlying gRPC HTTP request. gRPC-Web doesn't require a specific version and doesn't override the default unless specified.

### gRPC-Web and streaming

Traditional gRPC over HTTP/2 supports streaming in all directions. gRPC-Web offers limited support for streaming:

* gRPC-Web browser clients don't support calling client streaming and bidirectional streaming methods.
* gRPC-Web .NET clients don't support calling client streaming and bidirectional streaming methods over HTTP/1.1.
* ASP.NET Core gRPC services hosted on Azure App Service and IIS don't support bidirectional streaming.

When using gRPC-Web, we only recommend the use of unary methods and server streaming methods.

## Links

* [Documentation](https://docs.microsoft.com/aspnet/core/grpc/browser)
* [grpc-dotnet GitHub](https://github.com/grpc/grpc-dotnet)
