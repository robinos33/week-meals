import { QueryClient } from "@tanstack/react-query";

/** Client TanStack Query partagé. */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});
