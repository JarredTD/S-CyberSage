import { Tags } from 'aws-cdk-lib';
import type { IConstruct } from 'constructs';

/**
 * Applies the workspace ownership contract to a CyberSage component scope.
 *
 * @param scope - Construct scope receiving the ownership tags.
 * @param component - Stable functional component slug.
 */
export function applyResourceTags(scope: IConstruct, component: string): void {
  Tags.of(scope).add('Project', 's-cybersage');
  Tags.of(scope).add('Environment', 'prod');
  Tags.of(scope).add('Component', component);
  Tags.of(scope).add('ManagedBy', 'aws-cdk');
  Tags.of(scope).add('DeploymentBoundary', 'discord-cybersage');
}

/**
 * Overrides the functional component for a nested CyberSage resource.
 *
 * @param scope - Nested resource or construct scope.
 * @param component - Stable functional component slug.
 */
export function tagComponent(scope: IConstruct, component: string): void {
  Tags.of(scope).add('Component', component, { priority: 100 });
}
