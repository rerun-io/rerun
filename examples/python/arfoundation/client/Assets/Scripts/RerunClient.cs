using System;
using Cysharp.Net.Http;
using Google.Protobuf;
using Grpc.Net.Client;
using TMPro;
using UnityEngine;
using UnityEngine.UI;
using UnityEngine.XR.ARFoundation;
using RerunAR;

public class RerunClient : MonoBehaviour
{
    public Button connectButton;
    public Button startPauseButton;
    public TMP_InputField urlInput;
    public ARCameraManager cameraManager;
    public AROcclusionManager occlusionManager;

    private GrpcChannel _channel;
    private RerunARService.RerunARServiceClient _client;
    private Vector2Int _colorSampleSize;
    private string _sessionId;
    private bool _enabled = false;

    // Start is called before the first frame update
    void Start()
    {
        connectButton.onClick.AddListener(OnConnectButtonClick);
        startPauseButton.onClick.AddListener(OnStartPauseButtonClick);

        occlusionManager.environmentDepthTemporalSmoothingRequested = true;

        // http://192.168.2.1:8500
        var host = urlInput.text;

        // YetAnotherHttpHandler supports HTTP/2.
        var handler = new YetAnotherHttpHandler() {Http2Only = true};
        _channel = GrpcChannel.ForAddress(host, new GrpcChannelOptions()
        {
            HttpHandler = handler,
            MaxReceiveMessageSize = 4194304 * 10
        });
        _client = new RerunARService.RerunARServiceClient(_channel);
    }

    // Update is called once per frame
    void Update()
    {
        if (!_enabled) return;
        UploadFrame();
    }

    private void OnConnectButtonClick()
    {
        try
        {
            const int colorToDepthRatio = 1;
            cameraManager.TryGetIntrinsics(out var k);
            cameraManager.TryAcquireLatestCpuImage(out var colorImage);
            occlusionManager.TryAcquireEnvironmentDepthCpuImage(out var depthImage);

            _colorSampleSize = new Vector2Int(depthImage.dimensions.x * colorToDepthRatio,
                depthImage.dimensions.y * colorToDepthRatio);

            var response = _client.register(new RegisterRequest()
            {
                FocalLengthX = k.focalLength.x,
                FocalLengthY = k.focalLength.y,
                PrincipalPointX = k.principalPoint.x,
                PrincipalPointY = k.principalPoint.y,
                ColorResolutionX = colorImage.dimensions.x,
                ColorResolutionY = colorImage.dimensions.y,
                ColorSampleSizeX = _colorSampleSize.x,
                ColorSampleSizeY = _colorSampleSize.y,
                DepthResolutionX = depthImage.dimensions.x,
                DepthResolutionY = depthImage.dimensions.y
            });

            _sessionId = response.Message;

            Debug.Log(response.Message);

            colorImage.Dispose();
            depthImage.Dispose();
        }
        catch (Exception e)
        {
            // Try to catch any exceptions.
            // Network, device image, camera intrinsics
            Console.WriteLine(e);
        }
    }

    private void OnStartPauseButtonClick()
    {
        _enabled = !_enabled;
        startPauseButton.GetComponentInChildren<TMP_Text>().text = _enabled ? "Pause" : "Start";
    }

    private void UploadFrame()
    {
        var colorImage = new XRYCbCrColorImage(cameraManager, _colorSampleSize);
        var depthImage = new XRConfidenceFilteredDepthImage(occlusionManager, 0);

        const int transformLength = 3 * 4 * sizeof(float);
        var m = Camera.main!.transform.localToWorldMatrix;
        var cameraTransformBytes = new byte[transformLength];

        Buffer.BlockCopy(new[]
        {
            m.m00, m.m01, m.m02, m.m03,
            m.m10, m.m11, m.m12, m.m13,
            m.m20, m.m21, m.m22, m.m23
        }, 0, cameraTransformBytes, 0, transformLength);

        var response = _client.data_frame(new DataFrameRequest()
        {
            Uid = _sessionId,
            Color = ByteString.CopyFrom(colorImage.Encode()),
            Depth = ByteString.CopyFrom(depthImage.Encode()),
            Transform = ByteString.CopyFrom(cameraTransformBytes)
        });

        colorImage.Dispose();
        depthImage.Dispose();
    }

    private void OnDestroy()
    {
        _channel.Dispose();
    }
}
