using System;
using UnityEngine;
using UnityEngine.XR.ARFoundation;
using UnityEngine.XR.ARSubsystems;

namespace RerunAR
{
    internal interface IXRCpuImageEncodable
    {
        public byte[] Encode();
    }

    internal struct XRDepthImage : IXRCpuImageEncodable
    {
        private XRCpuImage _image;

        public XRDepthImage(AROcclusionManager occlusionManager)
        {
            occlusionManager.TryAcquireEnvironmentDepthCpuImage(out _image);
        }

        public byte[] Encode()
        {
            return _image.GetPlane(0).data.ToArray();
        }

        public void Dispose()
        {
            _image.Dispose();
        }
    }

    internal struct XRConfidenceFilteredDepthImage : IXRCpuImageEncodable
    {
        private XRCpuImage _depthImage;
        private XRCpuImage _confidenceImage;
        private readonly int _minConfidence;

        public Vector2Int Size()
        {
            return _depthImage.dimensions;
        }

        public XRConfidenceFilteredDepthImage(AROcclusionManager occlusionManager, int minConfidence = 1)
        {
            // occlusionManager.TryAcquireEnvironmentDepthCpuImage(out _depthImage);
            occlusionManager.TryAcquireSmoothedEnvironmentDepthCpuImage(out _depthImage);
            occlusionManager.TryAcquireEnvironmentDepthConfidenceCpuImage(out _confidenceImage);
            _minConfidence = minConfidence;
        }

        public byte[] Encode()
        {
            var depthValues = _depthImage.GetPlane(0).data.ToArray();
            var confidenceValues = _confidenceImage.GetPlane(0).data;

            for (var i = 0; i < confidenceValues.Length; i++)
            {
                // filter low confidence depth
                // convert to 1000, will be occluded by later calculation on edge
                var c = confidenceValues[i];
                if (c >= _minConfidence) continue;

                // Don't panic, these magic numbers represent number 1000 in float32 format
                depthValues[i * 4 + 0] = 0;
                depthValues[i * 4 + 1] = 0;
                depthValues[i * 4 + 2] = 0;
                depthValues[i * 4 + 3] = 0;
            }

            return depthValues;
        }

        public void Dispose()
        {
            _depthImage.Dispose();
            _confidenceImage.Dispose();
        }
    }

    internal struct XRYCbCrColorImage : IXRCpuImageEncodable
    {
        private XRCpuImage _image;
        private readonly float _scale;

        private readonly Vector2Int _nativeSize;
        private readonly Vector2Int _sampleSize;


        public XRYCbCrColorImage(ARCameraManager cameraManager, Vector2Int sampleSize)
        {
            cameraManager.TryAcquireLatestCpuImage(out _image);

            _nativeSize = _image.dimensions;
            this._sampleSize = sampleSize;
            _scale = this._sampleSize.x / (float) _nativeSize.x;
        }

        public byte[] Encode()
        {
            var size = _sampleSize.x * _sampleSize.y + 2 * (_sampleSize.x / 2 * _sampleSize.y / 2);
            var colorBytes = new byte[size];

            // Currently using nearest sampling, consider upgrade 
            // to bi-linear sampling for better anti-aliasing.
            var planeY = _image.GetPlane(0).data;
            for (var v = 0; v < _sampleSize.y; v++)
            {
                for (var u = 0; u < _sampleSize.x; u++)
                {
                    var iv = (int) (v / _scale);
                    var iu = (int) (u / _scale);
                    colorBytes[v * _sampleSize.x + u] = planeY[iv * _nativeSize.x + iu];
                }
            }

            var planeCbCr = _image.GetPlane(1).data;
            var offsetUV = _sampleSize.x * _sampleSize.y;
            for (var v = 0; v < _sampleSize.y / 2; v++)
            {
                for (var u = 0; u < _sampleSize.x / 2; u++)
                {
                    var iv = (int) (v / _scale);
                    var iu = (int) (u / _scale);

                    var sampleOffset = offsetUV + v * _sampleSize.x + u * 2;
                    var nativeOffset = iv * _nativeSize.x / 2 * 2 + iu * 2;

                    colorBytes[sampleOffset + 0] = planeCbCr[nativeOffset + 0];
                    colorBytes[sampleOffset + 1] = planeCbCr[nativeOffset + 1];
                }
            }

            return colorBytes;
        }

        public void Dispose()
        {
            _image.Dispose();
        }
    }
}