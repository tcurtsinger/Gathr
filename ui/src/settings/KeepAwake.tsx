import { Switch } from '../components/Switch';
import { Row } from './Row';
import { useSettings } from './saved';

/** Settings > General: whether this computer stays awake while agents work. */
export function KeepAwake() {
  const { saved, save } = useSettings();
  if (!saved || saved.keep_awake === undefined) return null;
  return (
    <Row name={<span id="awake-label">Keep awake</span>} line="While agents work." lineId="awake-line">
      <Switch
        on={saved.keep_awake}
        labelledBy="awake-label"
        describedBy="awake-line"
        onChange={(on) => save({ keep_awake: on })}
      />
    </Row>
  );
}
