import { common } from '@kit.AbilityKit'
import { taskpool } from '@kit.ArkTS'
import probe from 'libtantivy_probe.so'

@Concurrent
function RunBenchmark(root: string, phase: string): string {
  return probe.run(root, phase)
}

@Entry
@Component
struct Index {
  @State result: string = 'Running SDK benchmark'
  aboutToAppear(): void {
    const context = getContext(this) as common.UIAbilityContext
    const phase: string = AppStorage.get<string>('phase') ?? 'prepare_fast'
    const runId: string = AppStorage.get<string>('runId') ?? 'benchmark-01'
    const task = new taskpool.Task(RunBenchmark, context.filesDir + '/' + runId, phase)
    taskpool.execute(task).then((value: Object) => {
      this.result = value as string
    }).catch((error: Error) => {
      this.result = error.message
    })
  }
  build() {
    Column({ space: 20 }) {
      Text('Tantivy SDK benchmark').fontSize(24)
      Text(this.result).fontSize(16)
    }.padding(24).width('100%').height('100%').justifyContent(FlexAlign.Center)
  }
}
