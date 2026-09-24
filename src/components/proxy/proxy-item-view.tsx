import { CheckCircleOutlineRounded } from '@mui/icons-material'
import {
  alpha,
  Box,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  styled,
  type SxProps,
  type Theme,
} from '@mui/material'
import { useTranslation } from 'react-i18next'

import { BaseLoading } from '@/components/base/base-loading'

interface Props {
  name: string
  type: string
  selected: boolean
  disabled?: boolean
  showType?: boolean
  now?: string
  udp?: boolean
  xudp?: boolean
  tfo?: boolean
  mptcp?: boolean
  smux?: boolean
  delayValue: number
  delayText?: string
  delayColor?: string
  isPreset?: boolean
  sx?: SxProps<Theme>
  onClick?: () => void
  onDelay?: () => void | Promise<void>
}

const Widget = styled(Box)(() => ({
  padding: '3px 6px',
  fontSize: 14,
  borderRadius: '4px',
}))

const TypeBox = styled('span')(({ theme }) => ({
  display: 'inline-block',
  border: '1px solid #ccc',
  borderColor: alpha(theme.palette.text.secondary, 0.36),
  color: alpha(theme.palette.text.secondary, 0.42),
  borderRadius: 4,
  fontSize: 10,
  marginRight: '4px',
  padding: '0 2px',
  lineHeight: 1.25,
}))

export const ProxyItemView = ({
  name,
  type,
  selected,
  disabled = false,
  showType = true,
  now,
  udp,
  xudp,
  tfo,
  mptcp,
  smux,
  delayValue,
  delayText,
  delayColor,
  isPreset = false,
  sx,
  onClick,
  onDelay,
}: Props) => {
  const { t } = useTranslation()

  return (
    <ListItem sx={sx}>
      <ListItemButton
        dense
        disabled={disabled}
        selected={!disabled && selected}
        onClick={disabled ? undefined : onClick}
        sx={[
          { borderRadius: 1 },
          ({ palette: { mode, primary } }) => {
            const bgcolor = mode === 'light' ? '#ffffff' : '#24252f'
            const selectColor = mode === 'light' ? primary.main : primary.light
            const showDelay = delayValue > 0
            return {
              '&:hover .the-check': { display: !showDelay ? 'block' : 'none' },
              '&:hover .the-delay': { display: showDelay ? 'block' : 'none' },
              '&:hover .the-icon': { display: 'none' },
              '&.Mui-selected': {
                width: 'calc(100% + 3px)',
                marginLeft: '-3px',
                borderLeft: `3px solid ${selectColor}`,
                bgcolor:
                  mode === 'light'
                    ? alpha(primary.main, 0.15)
                    : alpha(primary.main, 0.35),
              },
              backgroundColor: bgcolor,
              marginBottom: '8px',
              height: '40px',
            }
          },
        ]}
      >
        <ListItemText
          title={name}
          secondary={
            <>
              <Box
                sx={{
                  display: 'inline-block',
                  marginRight: '8px',
                  fontSize: '14px',
                  color: 'text.primary',
                }}
              >
                {name}
                {showType && now && ` - ${now}`}
              </Box>
              {showType && <TypeBox>{type}</TypeBox>}
              {showType && udp && <TypeBox>UDP</TypeBox>}
              {showType && xudp && <TypeBox>XUDP</TypeBox>}
              {showType && tfo && <TypeBox>TFO</TypeBox>}
              {showType && mptcp && <TypeBox>MPTCP</TypeBox>}
              {showType && smux && <TypeBox>SMUX</TypeBox>}
            </>
          }
        />

        <ListItemIcon
          sx={{
            justifyContent: 'flex-end',
            color: 'primary.main',
            display: isPreset ? 'none' : '',
          }}
        >
          {!disabled && delayValue === -2 && (
            <Widget>
              <BaseLoading />
            </Widget>
          )}
          {!disabled && delayValue !== -2 && (
            <Widget
              className="the-check"
              onClick={(event) => {
                event.preventDefault()
                event.stopPropagation()
                void onDelay?.()
              }}
              sx={({ palette }) => ({
                display: 'none',
                ':hover': { bgcolor: alpha(palette.primary.main, 0.15) },
              })}
            >
              {t('shared.actions.check')}
            </Widget>
          )}
          {!disabled && delayValue > 0 && (
            <Widget
              className="the-delay"
              onClick={(event) => {
                event.preventDefault()
                event.stopPropagation()
                void onDelay?.()
              }}
              sx={({ palette }) => ({
                color: delayColor,
                ':hover': { bgcolor: alpha(palette.primary.main, 0.15) },
              })}
            >
              {delayText ?? `${delayValue} ms`}
            </Widget>
          )}
          {!disabled && delayValue !== -2 && delayValue <= 0 && selected && (
            <CheckCircleOutlineRounded
              className="the-icon"
              sx={{ fontSize: 16 }}
            />
          )}
        </ListItemIcon>
      </ListItemButton>
    </ListItem>
  )
}
