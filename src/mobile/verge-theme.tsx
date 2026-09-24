import { CssBaseline, ThemeProvider, createTheme, type Shadows } from '@mui/material'
import { type ReactNode, useEffect, useMemo, useState } from 'react'

const defaultTheme = {
  primary_color: '#007AFF',
  secondary_color: '#FC9B76',
  primary_text: '#000000',
  secondary_text: '#3C3C4399',
  info_color: '#007AFF',
  error_color: '#FF3B30',
  warning_color: '#FF9500',
  success_color: '#06943D',
  background_color: '#F5F5F5',
  font_family:
    '-apple-system, BlinkMacSystemFont, "Microsoft YaHei UI", "Microsoft YaHei", Roboto, "Helvetica Neue", Arial, sans-serif, "Apple Color Emoji"',
}

const defaultDarkTheme = {
  ...defaultTheme,
  primary_color: '#0A84FF',
  secondary_color: '#FF9F0A',
  primary_text: '#FFFFFF',
  background_color: '#2E303D',
  secondary_text: '#EBEBF599',
  info_color: '#0A84FF',
  error_color: '#FF453A',
  warning_color: '#FF9F0A',
  success_color: '#30D158',
}

export type MobileThemeMode = 'light' | 'dark' | 'system'

export function VergeMobileTheme({
  children,
  mode = 'system',
}: {
  children: ReactNode
  mode?: MobileThemeMode
}) {
  const [systemMode, setSystemMode] = useState<'light' | 'dark'>(() =>
    window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light',
  )

  useEffect(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const update = () => setSystemMode(media.matches ? 'dark' : 'light')
    media.addEventListener('change', update)
    return () => media.removeEventListener('change', update)
  }, [])

  const resolvedMode = mode === 'system' ? systemMode : mode

  const theme = useMemo(() => {
    const dt = resolvedMode === 'light' ? defaultTheme : defaultDarkTheme
    const muiTheme = createTheme({
      breakpoints: {
        values: { xs: 0, sm: 650, md: 900, lg: 1200, xl: 1536 },
      },
      palette: {
        mode: resolvedMode,
        primary: { main: dt.primary_color },
        secondary: { main: dt.secondary_color },
        info: { main: dt.info_color },
        error: { main: dt.error_color },
        warning: { main: dt.warning_color },
        success: { main: dt.success_color },
        text: { primary: dt.primary_text, secondary: dt.secondary_text },
        background: {
          paper: dt.background_color,
          default: dt.background_color,
        },
      },
      shadows: Array(25).fill('none') as Shadows,
      typography: { fontFamily: dt.font_family },
    })

    const root = document.documentElement
    root.style.setProperty(
      '--divider-color',
      resolvedMode === 'light'
        ? 'rgba(0, 0, 0, 0.06)'
        : 'rgba(255, 255, 255, 0.06)',
    )
    root.style.setProperty(
      '--background-color',
      resolvedMode === 'light' ? '#ECECEC' : dt.background_color,
    )
    root.style.setProperty(
      '--mobile-content-bg',
      resolvedMode === 'light' ? '#ECECEC' : '#1e1f27',
    )
    root.style.setProperty(
      '--verge-panel-bg',
      resolvedMode === 'light' ? '#ffffff' : '#282a36',
    )
    root.style.setProperty('--primary-main', muiTheme.palette.primary.main)
    root.style.setProperty(
      '--scroller-color',
      resolvedMode === 'light' ? '#90939980' : '#555555',
    )
    root.style.colorScheme = resolvedMode
    return muiTheme
  }, [resolvedMode])

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      {children}
    </ThemeProvider>
  )
}
