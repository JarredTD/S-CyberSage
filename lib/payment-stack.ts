import { Stack, StackProps, RemovalPolicy } from "aws-cdk-lib";
import { Construct } from "constructs";
import { Table, AttributeType, BillingMode } from "aws-cdk-lib/aws-dynamodb";

export class PaymentStack extends Stack {
  public readonly guildSubscriptionsTable: Table;

  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);

    this.guildSubscriptionsTable = new Table(this, "GuildSubscriptionsTable", {
      tableName: "GuildSubscriptions",
      partitionKey: { name: "guild_id", type: AttributeType.STRING },
      sortKey: { name: "subscription_key", type: AttributeType.STRING },
      billingMode: BillingMode.PAY_PER_REQUEST,
      pointInTimeRecovery: true,
      removalPolicy: RemovalPolicy.DESTROY,
    });
  }
}
