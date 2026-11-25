// The /charts components that run in the browser, as the Astro pages hydrate them. Each island is
// its own React root, so the ones that keep state in the URL (?ex, ?seed, ?theme, ?q ...) get a nuqs
// adapter each; nuqs syncs them through one emitter, so Run in the example list drives the stage.

import type { ComponentProps, ComponentType } from "react"
import { NuqsAdapter } from "nuqs/adapters/react"
import { AllGrid } from "./all-grid"
import { ExampleList } from "./example-list"
import { Stage } from "./stage"
import { StageProvider } from "./stage-provider"

const withUrlState =
  <P extends object>(Island: ComponentType<P>) =>
  (props: P) => (
    <NuqsAdapter>
      <Island {...props} />
    </NuqsAdapter>
  )

export const LiveStage = withUrlState((props: ComponentProps<typeof Stage>) => (
  <StageProvider>
    <Stage {...props} />
  </StageProvider>
))

export const Examples = withUrlState(ExampleList)

export const AllCharts = withUrlState(AllGrid)
