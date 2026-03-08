#include "common.hlsl"
#include <d3d11.h>

cbuffer PerFrame : register(b0) {
    float4x4 gView;
    float4x4 gProjection;
    float3   gCameraPos;
    float    gTime;
};

cbuffer PerObject : register(b1) {
    float4x4 gWorld;
    float4   gColor;
};

Texture2D    gAlbedoTex  : register(t0);
Texture2D    gNormalTex  : register(t1);
SamplerState gLinearSamp : register(s0);

struct VSInput {
    float3 Position : POSITION;
    float3 Normal   : NORMAL;
    float2 TexCoord : TEXCOORD0;
};

struct PSInput {
    float4 Position : SV_POSITION;
    float3 WorldPos : TEXCOORD0;
    float3 Normal   : TEXCOORD1;
    float2 TexCoord : TEXCOORD2;
};

float3 ComputeLighting(float3 normal, float3 lightDir, float3 viewDir) {
    float diff = max(dot(normal, lightDir), 0.0f);
    float3 reflectDir = reflect(-lightDir, normal);
    float spec = pow(max(dot(viewDir, reflectDir), 0.0f), 32.0f);
    return diff * float3(1, 1, 1) + spec * float3(0.5, 0.5, 0.5);
}

PSInput VSMain(VSInput input) {
    PSInput output;
    float4 worldPos = mul(float4(input.Position, 1.0f), gWorld);
    output.WorldPos = worldPos.xyz;
    output.Position = mul(mul(worldPos, gView), gProjection);
    output.Normal   = mul(input.Normal, (float3x3)gWorld);
    output.TexCoord = input.TexCoord;
    return output;
}

float4 PSMain(PSInput input) : SV_TARGET {
    float3 norm     = normalize(input.Normal);
    float3 lightDir = normalize(float3(1, 2, 1));
    float3 viewDir  = normalize(gCameraPos - input.WorldPos);
    float4 albedo   = gAlbedoTex.Sample(gLinearSamp, input.TexCoord);
    float3 lighting = ComputeLighting(norm, lightDir, viewDir);
    return float4(albedo.rgb * lighting, albedo.a) * gColor;
}
