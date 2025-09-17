import {
  Stack,
  StackProps,
  Duration,
  CfnOutput,
  RemovalPolicy,
} from "aws-cdk-lib";
import { Construct } from "constructs";
import { RetentionDays, LogGroup } from "aws-cdk-lib/aws-logs";
import { Function, Runtime, Code, Architecture } from "aws-cdk-lib/aws-lambda";
import {
  HttpApi,
  HttpMethod,
  CorsHttpMethod,
  CfnStage,
} from "aws-cdk-lib/aws-apigatewayv2";
import { HttpLambdaIntegration } from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { Table, AttributeType, BillingMode } from "aws-cdk-lib/aws-dynamodb";
import { Secret } from "aws-cdk-lib/aws-secretsmanager";
import { join } from "path";

export class CyberSageCdkStack extends Stack {
  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);

    const roleMappingsTable = new Table(this, "RoleMappingsTable", {
      partitionKey: { name: "roleName", type: AttributeType.STRING },
      billingMode: BillingMode.PAY_PER_REQUEST,
      removalPolicy: RemovalPolicy.DESTROY,
    });

    const discordTokenSecret = new Secret(this, "DiscordTokenSecret", {
      description: "Discord Bot Token",
      generateSecretString: {
        secretStringTemplate: JSON.stringify({}),
        generateStringKey: "token",
      },
    });

    const discordPublicKeySecret = new Secret(this, "DiscordPublicKeySecret", {
      description: "Discord Public Key",
      generateSecretString: {
        secretStringTemplate: JSON.stringify({}),
        generateStringKey: "key",
      },
    });

    const botLogGroup = new LogGroup(this, "DiscordBotLogGroup", {
      retention: RetentionDays.ONE_WEEK,
      logGroupName: "/aws/lambda/discord-bot-handler",
      removalPolicy: RemovalPolicy.DESTROY,
    });

    const bootstrapDir = join(__dirname, "../lambda/bootstrap");
    const discordBotHandler = new Function(this, "DiscordBotHandler", {
      runtime: Runtime.PROVIDED_AL2,
      architecture: Architecture.ARM_64,
      handler: "bootstrap",
      code: Code.fromAsset(bootstrapDir),
      memorySize: 256,
      timeout: Duration.seconds(10),
      environment: {
        ROLE_MAPPINGS_TABLE_NAME: roleMappingsTable.tableName,
        DISCORD_TOKEN_SECRET_ARN: discordTokenSecret.secretArn,
        DISCORD_PUBLIC_KEY_SECRET_ARN: discordPublicKeySecret.secretArn,
      },
      logGroup: botLogGroup,
    });

    roleMappingsTable.grantReadData(discordBotHandler);
    discordTokenSecret.grantRead(discordBotHandler);
    discordPublicKeySecret.grantRead(discordBotHandler);

    const api = new HttpApi(this, "DiscordBotApi", {
      description: "HTTP API for Discord bot interactions",
      corsPreflight: {
        allowHeaders: [
          "Content-Type",
          "X-Amz-Date",
          "Authorization",
          "X-Api-Key",
        ],
        allowMethods: [CorsHttpMethod.POST],
        allowOrigins: ["*"],
      },
      createDefaultStage: false,
    });

    new CfnStage(this, "ProdStage", {
      apiId: api.apiId,
      stageName: "prod",
      autoDeploy: true,
      defaultRouteSettings: {
        throttlingRateLimit: 50,
        throttlingBurstLimit: 100,
      },
    });

    const lambdaIntegration = new HttpLambdaIntegration(
      "DiscordBotIntegration",
      discordBotHandler
    );

    api.addRoutes({
      path: "/",
      methods: [HttpMethod.POST],
      integration: lambdaIntegration,
    });

    new CfnOutput(this, "ApiEndpoint", {
      value: `https://${api.apiId}.execute-api.${this.region}.amazonaws.com/prod/`,
      description: "API Gateway endpoint URL for Discord interactions",
    });
  }
}
