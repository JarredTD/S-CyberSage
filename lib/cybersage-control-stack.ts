import { Stack, Duration, CfnOutput, RemovalPolicy } from 'aws-cdk-lib';
import type { StackProps } from 'aws-cdk-lib';
import type { Construct } from 'constructs';
import { RetentionDays, LogGroup } from 'aws-cdk-lib/aws-logs';
import { NagSuppressions } from 'cdk-nag';
import { Function, Runtime, Code, Architecture } from 'aws-cdk-lib/aws-lambda';
import { HttpApi, HttpMethod, CfnStage } from 'aws-cdk-lib/aws-apigatewayv2';
import { HttpLambdaIntegration } from 'aws-cdk-lib/aws-apigatewayv2-integrations';
import type { Table } from 'aws-cdk-lib/aws-dynamodb';
import { Secret } from 'aws-cdk-lib/aws-secretsmanager';
import { join } from 'path';

/** Properties required to create the control-plane stack. */
interface Props extends StackProps {
  mainTable: Table;
}

/** Deploys the Discord interaction API, Lambda function, and application secrets. */
export class CyberSageControlStack extends Stack {
  constructor(scope: Construct, id: string, props: Props) {
    super(scope, id, props);

    const discordTokenSecret = new Secret(this, 'DiscordTokenSecret', {
      description: 'Discord Bot Token',
      generateSecretString: {
        secretStringTemplate: JSON.stringify({}),
        generateStringKey: 'token',
      },
    });

    const discordPublicKeySecret = new Secret(this, 'DiscordPublicKeySecret', {
      description: 'Discord Public Key',
      generateSecretString: {
        secretStringTemplate: JSON.stringify({}),
        generateStringKey: 'key',
      },
    });

    const logGroup = new LogGroup(this, 'DiscordBotLogGroup', {
      logGroupName: '/aws/lambda/discord-bot-handler',
      retention: RetentionDays.ONE_WEEK,
      removalPolicy: RemovalPolicy.DESTROY,
    });

    const lambdaZip = join(__dirname, '../lambda/s-cybersage-rs/bootstrap.zip');

    const discordBotHandler = new Function(this, 'DiscordBotHandler', {
      runtime: Runtime.PROVIDED_AL2,
      architecture: Architecture.ARM_64,
      handler: 'bootstrap',
      code: Code.fromAsset(lambdaZip),
      memorySize: 256,
      timeout: Duration.seconds(10),
      logGroup,
      environment: {
        MAIN_TABLE_NAME: props.mainTable.tableName,
        DISCORD_TOKEN_SECRET_ARN: discordTokenSecret.secretArn,
        DISCORD_PUBLIC_KEY_SECRET_ARN: discordPublicKeySecret.secretArn,
        RUST_LOG: 'info',
      },
    });

    props.mainTable.grantReadWriteData(discordBotHandler);
    discordTokenSecret.grantRead(discordBotHandler);
    discordPublicKeySecret.grantRead(discordBotHandler);

    const api = new HttpApi(this, 'DiscordBotApi', {
      description: 'HTTP API for Discord bot interactions',
      createDefaultStage: false,
    });

    new CfnStage(this, 'ProdStage', {
      apiId: api.apiId,
      stageName: 'prod',
      autoDeploy: true,
      defaultRouteSettings: {
        throttlingRateLimit: 50,
        throttlingBurstLimit: 100,
      },
    });

    api.addRoutes({
      path: '/',
      methods: [HttpMethod.POST],
      integration: new HttpLambdaIntegration('DiscordBotIntegration', discordBotHandler),
    });

    new CfnOutput(this, 'ApiEndpoint', {
      value: `https://${api.apiId}.execute-api.${this.region}.amazonaws.com/prod/`,
    });

    NagSuppressions.addResourceSuppressions(discordTokenSecret, [
      {
        id: 'AwsSolutions-SMG4',
        reason:
          'Rotating a Discord bot token requires coordinated revocation and configuration changes in Discord; rotation is performed manually through the Discord Developer Portal.',
      },
    ]);
    NagSuppressions.addResourceSuppressions(discordPublicKeySecret, [
      {
        id: 'AwsSolutions-SMG4',
        reason:
          'The Discord application public key is a verification value controlled by Discord and must be updated only when Discord rotates it.',
      },
    ]);
    NagSuppressions.addResourceSuppressionsByPath(
      this,
      '/S-CyberSageControlStack/DiscordBotHandler/ServiceRole/Resource',
      [
        {
          id: 'AwsSolutions-IAM4',
          appliesTo: [
            'Policy::arn:<AWS::Partition>:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole',
          ],
          reason:
            'CDK attaches this AWS-managed basic Lambda logging policy; its permissions are limited to CloudWatch Logs operations required by the Lambda runtime.',
        },
      ],
    );
    NagSuppressions.addResourceSuppressionsByPath(
      this,
      '/S-CyberSageControlStack/DiscordBotHandler/ServiceRole/DefaultPolicy/Resource',
      [
        {
          id: 'AwsSolutions-IAM5',
          appliesTo: ['Resource::<CyberSageMainTableF82846DC.Arn>/index/*'],
          reason:
            'DynamoDB secondary-index ARNs require a wildcard suffix; access is scoped to this stack’s single table and its indexes.',
        },
      ],
    );
    NagSuppressions.addResourceSuppressionsByPath(
      this,
      '/S-CyberSageControlStack/DiscordBotApi/POST--/Resource',
      [
        {
          id: 'AwsSolutions-APIG4',
          reason:
            'Discord sends unauthenticated public webhooks. The Lambda verifies Discord’s Ed25519 request signature before processing every interaction.',
        },
      ],
    );
    NagSuppressions.addResourceSuppressionsByPath(this, '/S-CyberSageControlStack/ProdStage', [
      {
        id: 'AwsSolutions-APIG1',
        reason:
          'This low-traffic webhook does not retain API access logs to avoid recurring CloudWatch Logs charges; the Lambda retains application-level logs for troubleshooting.',
      },
    ]);
  }
}
