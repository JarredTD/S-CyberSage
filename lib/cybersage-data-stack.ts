import { Stack, RemovalPolicy, CfnOutput } from 'aws-cdk-lib';
import type { StackProps } from 'aws-cdk-lib';
import type { Construct } from 'constructs';
import { Table, AttributeType, BillingMode, ProjectionType } from 'aws-cdk-lib/aws-dynamodb';

/** Deploys the persistent storage used by the Discord bot. */
export class CyberSageDataStack extends Stack {
  public readonly mainTable: Table;

  constructor(scope: Construct, id: string, props?: StackProps) {
    super(scope, id, props);

    this.mainTable = new Table(this, 'CyberSageMainTable', {
      tableName: 'CyberSageMain',
      partitionKey: { name: 'PK', type: AttributeType.STRING },
      sortKey: { name: 'SK', type: AttributeType.STRING },
      billingMode: BillingMode.PAY_PER_REQUEST,
      removalPolicy: RemovalPolicy.DESTROY,
    });

    this.mainTable.addGlobalSecondaryIndex({
      indexName: 'LookupByRoleName',
      partitionKey: {
        name: 'role_name_lookup_pk',
        type: AttributeType.STRING,
      },
      sortKey: {
        name: 'role_name_lookup_sk',
        type: AttributeType.STRING,
      },
      projectionType: ProjectionType.ALL,
    });

    this.mainTable.addGlobalSecondaryIndex({
      indexName: 'LookupByRoleId',
      partitionKey: {
        name: 'role_id_lookup_pk',
        type: AttributeType.STRING,
      },
      sortKey: {
        name: 'role_id_lookup_sk',
        type: AttributeType.STRING,
      },
      projectionType: ProjectionType.ALL,
    });

    this.mainTable.addGlobalSecondaryIndex({
      indexName: 'LookupByEntityType',
      partitionKey: {
        name: 'entity_type_lookup_pk',
        type: AttributeType.STRING,
      },
      sortKey: {
        name: 'entity_type_lookup_sk',
        type: AttributeType.STRING,
      },
      projectionType: ProjectionType.ALL,
    });

    new CfnOutput(this, 'TableName', {
      value: this.mainTable.tableName,
    });
  }
}
