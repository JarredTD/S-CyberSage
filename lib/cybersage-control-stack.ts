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
import { HttpApi, HttpMethod, CfnStage } from "aws-cdk-lib/aws-apigatewayv2";
import { HttpLambdaIntegration } from "aws-cdk-lib/aws-apigatewayv2-integrations";
import { Table } from "aws-cdk-lib/aws-dynamodb";
import { Secret } from "aws-cdk-lib/aws-secretsmanager";
import { join } from "path";

interface Props extends StackProps {
  mainTable: Table;
}

export class CyberSageControlStack extends Stack {
  constructor(scope: Construct, id: string, props: Props) {
    super(scope, id, props);

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

    const logGroup = new LogGroup(this, "DiscordBotLogGroup", {
      logGroupName: "/aws/lambda/discord-bot-handler",
      retention: RetentionDays.ONE_WEEK,
      removalPolicy: RemovalPolicy.DESTROY,
    });

    const lambdaZip = join(__dirname, "../lambda/s-cybersage-rs/bootstrap.zip");

    const discordBotHandler = new Function(this, "DiscordBotHandler", {
      runtime: Runtime.PROVIDED_AL2,
      architecture: Architecture.ARM_64,
      handler: "bootstrap",
      code: Code.fromAsset(lambdaZip),
      memorySize: 256,
      timeout: Duration.seconds(10),
      logGroup,
      environment: {
        MAIN_TABLE_NAME: props.mainTable.tableName,
        DISCORD_TOKEN_SECRET_ARN: discordTokenSecret.secretArn,
        DISCORD_PUBLIC_KEY_SECRET_ARN: discordPublicKeySecret.secretArn,
        RUST_LOG: "info",
      },
    });

    props.mainTable.grantReadWriteData(discordBotHandler);
    discordTokenSecret.grantRead(discordBotHandler);
    discordPublicKeySecret.grantRead(discordBotHandler);

    const api = new HttpApi(this, "DiscordBotApi", {
      description: "HTTP API for Discord bot interactions",
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

    api.addRoutes({
      path: "/",
      methods: [HttpMethod.POST],
      integration: new HttpLambdaIntegration(
        "DiscordBotIntegration",
        discordBotHandler,
      ),
    });

    new CfnOutput(this, "ApiEndpoint", {
      value: `https://${api.apiId}.execute-api.${this.region}.amazonaws.com/prod/`,
    });
  }
}
