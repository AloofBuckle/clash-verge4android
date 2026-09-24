import { ArrowDropDown } from '@mui/icons-material'
import {
  Box,
  ButtonBase,
  ClickAwayListener,
  MenuList,
  Paper,
  Popper,
  type PopperProps,
  alpha,
} from '@mui/material'
import { useEffect, useId, useMemo, useRef, type ReactNode } from 'react'

const PROXY_MENU_MAX_HEIGHT = 500
const PROXY_MENU_VIEWPORT_MARGIN = 8

const proxyMenuModifiers: PopperProps['modifiers'] = [
  {
    name: 'menuSize',
    enabled: true,
    phase: 'beforeRead',
    fn: ({ state }) => {
      const popper = state.elements.popper
      const anchor = state.elements.reference.getBoundingClientRect()
      const viewportHeight = popper.ownerDocument.documentElement.clientHeight
      const availableHeight =
        Math.max(anchor.top, viewportHeight - anchor.bottom) -
        PROXY_MENU_VIEWPORT_MARGIN
      popper.style.width = `${anchor.width}px`
      popper.style.setProperty(
        '--proxy-menu-max-height',
        `${Math.max(0, Math.min(PROXY_MENU_MAX_HEIGHT, availableHeight))}px`,
      )
      state.rects.popper.width = popper.offsetWidth
      state.rects.popper.height = popper.offsetHeight
    },
  },
  {
    name: 'showMenu',
    enabled: true,
    phase: 'beforeWrite',
    requires: ['computeStyles'],
    fn: ({ state }) => {
      state.styles.popper.visibility = 'visible'
    },
  },
]

export interface PersistentSelectProps {
  label: string
  displayValue: ReactNode
  open: boolean
  disabled: boolean
  keepOpenRef: React.RefObject<HTMLElement | null>
  onOpenChange: (open: boolean) => void
  renderOptions: () => ReactNode
}

export const PersistentSelect = ({
  label,
  displayValue,
  open,
  disabled,
  keepOpenRef,
  onOpenChange,
  renderOptions,
}: PersistentSelectProps) => {
  const id = useId()
  const labelId = `${id}-label`
  const valueId = `${id}-value`
  const listboxId = `${id}-listbox`
  const anchorRef = useRef<HTMLButtonElement>(null)
  const listRef = useRef<HTMLUListElement>(null)

  const popperOptions = useMemo<PopperProps['popperOptions']>(
    () => ({
      onFirstUpdate: () => {
        const list = listRef.current
        const paper = list?.parentElement
        const item =
          list?.querySelector<HTMLElement>(
            '[aria-selected="true"]:not([aria-disabled="true"])',
          ) ??
          list?.querySelector<HTMLElement>(
            '[role="option"]:not([aria-disabled="true"])',
          )
        if (!item || !paper) return

        item.focus({ preventScroll: true })
        paper.scrollTop +=
          item.getBoundingClientRect().top -
          paper.getBoundingClientRect().top -
          (paper.clientHeight - item.offsetHeight) / 2
      },
    }),
    [],
  )

  useEffect(() => {
    if (!open) return

    const closeFromKeyboard = (event: KeyboardEvent) => {
      if (event.key === 'Tab') {
        onOpenChange(false)
      } else if (event.key === 'Escape') {
        onOpenChange(false)
        anchorRef.current?.focus({ preventScroll: true })
      }
    }
    document.addEventListener('keydown', closeFromKeyboard)
    return () => document.removeEventListener('keydown', closeFromKeyboard)
  }, [onOpenChange, open])

  return (
    <ClickAwayListener
      onClickAway={(event) => {
        if (!open) return
        const target = event.target
        if (target instanceof Node && keepOpenRef.current?.contains(target)) return
        onOpenChange(false)
      }}
    >
      <Box>
        <Box
          sx={{
            position: 'relative',
            borderRadius: 1,
            '& fieldset': {
              borderColor: (theme) =>
                alpha(theme.palette.text.primary, disabled ? 0.12 : 0.23),
            },
            ...(!disabled && {
              '&:hover fieldset': { borderColor: 'text.primary' },
              '&:focus-within fieldset': {
                borderColor: 'primary.main',
                borderWidth: 2,
              },
              '&:focus-within legend': { color: 'primary.main' },
            }),
          }}
        >
          <ButtonBase
            ref={anchorRef}
            type="button"
            role="combobox"
            aria-labelledby={`${labelId} ${valueId}`}
            aria-haspopup="listbox"
            aria-controls={open ? listboxId : undefined}
            aria-expanded={open}
            disabled={disabled}
            onClick={() => onOpenChange(!open)}
            onKeyDown={(event) => {
              if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
                event.preventDefault()
                onOpenChange(true)
              }
            }}
            sx={{
              width: '100%',
              height: 40,
              px: 1.75,
              borderRadius: 'inherit',
              justifyContent: 'space-between',
              textAlign: 'left',
              color: disabled ? 'text.disabled' : 'text.primary',
            }}
          >
            <Box id={valueId} sx={{ minWidth: 0, flex: 1 }}>
              {displayValue}
            </Box>
            <ArrowDropDown
              sx={{
                ml: 1,
                color: disabled ? 'action.disabled' : 'action.active',
                transform: open ? 'rotate(180deg)' : undefined,
              }}
            />
          </ButtonBase>
          <Box
            component="fieldset"
            sx={{
              position: 'absolute',
              inset: '-5px 0 0',
              m: 0,
              px: 1,
              minWidth: 0,
              border: '1px solid',
              borderRadius: 'inherit',
              pointerEvents: 'none',
            }}
          >
            <Box
              component="legend"
              id={labelId}
              sx={{
                px: 0.5,
                typography: 'caption',
                lineHeight: '11px',
                color: disabled ? 'text.disabled' : 'text.secondary',
              }}
            >
              {label}
            </Box>
          </Box>
        </Box>

        <Popper
          open={open}
          anchorEl={anchorRef.current}
          placement="bottom-start"
          modifiers={proxyMenuModifiers}
          popperOptions={popperOptions}
          sx={{ visibility: 'hidden', zIndex: (theme) => theme.zIndex.modal }}
        >
          <Paper
            elevation={8}
            sx={{
              maxHeight: `var(--proxy-menu-max-height, ${PROXY_MENU_MAX_HEIGHT}px)`,
              overflow: 'auto',
            }}
          >
            <MenuList
              ref={listRef}
              id={listboxId}
              role="listbox"
              aria-labelledby={labelId}
              variant="selectedMenu"
            >
              {open && renderOptions()}
            </MenuList>
          </Paper>
        </Popper>
      </Box>
    </ClickAwayListener>
  )
}
