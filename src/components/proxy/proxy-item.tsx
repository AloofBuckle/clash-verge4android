import { type SxProps, type Theme } from '@mui/material'

import { useProxyDelayState } from '@/hooks/use-proxy-delay-state'
import delayManager from '@/services/delay'
import {
  memberDetails,
  type ProxyGroupView,
  type ResolvedProxyMember,
} from '@/types/proxy-view'

import { ProxyItemView } from './proxy-item-view'

interface Props {
  group: ProxyGroupView
  member: ResolvedProxyMember
  selected: boolean
  showType?: boolean
  sx?: SxProps<Theme>
  onClick?: (member: ResolvedProxyMember) => void
}

export const ProxyItem = ({
  group,
  member,
  selected,
  showType = true,
  sx,
  onClick,
}: Props) => {
  const details = memberDetails(member)
  const unresolved = member.kind === 'unresolved'
  const name = member.ref.name
  const type = unresolved ? member.ref.reason : (details?.type ?? '')
  const now = member.kind === 'group' ? member.group.now : undefined
  const { delayValue, isPreset, timeout, onDelay } = useProxyDelayState(
    member,
    group.name,
  )

  return (
    <ProxyItemView
      name={name}
      type={type}
      selected={selected}
      disabled={unresolved}
      showType={showType}
      now={now}
      udp={!unresolved && details?.udp}
      xudp={!unresolved && details?.xudp}
      tfo={!unresolved && details?.tfo}
      mptcp={!unresolved && details?.mptcp}
      smux={!unresolved && details?.smux}
      delayValue={delayValue}
      delayText={
        delayValue > 0 ? delayManager.formatDelay(delayValue, timeout) : undefined
      }
      delayColor={
        delayValue > 0
          ? delayManager.formatDelayColor(delayValue, timeout)
          : undefined
      }
      isPreset={isPreset}
      sx={sx}
      onClick={unresolved ? undefined : () => onClick?.(member)}
      onDelay={onDelay}
    />
  )
}
