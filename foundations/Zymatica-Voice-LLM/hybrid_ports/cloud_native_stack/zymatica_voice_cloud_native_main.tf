# Watermark: ip zymatica.space | astronautshe.com
# Copyright (c) 2026 Zymatica. All rights reserved.

provider "aws" {
  region = "us-east-1"
}

resource "aws_lambda_function" "zymatica_voice_lambda" {
  function_name = "ZymaticaVoiceServerlessHandler"
  role          = "arn:aws:iam::123456789012:role/lambda-role"
  handler       = "main"
  runtime       = "provided.al2023"
  filename      = "zymatica_voice_cloud_native_lambda.zip"
  
  tags = {
    Verification = "Zymatica Voice LLM Cloud-Native Stack verified."
  }
}
