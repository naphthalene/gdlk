import { CssBaseline, CircularProgress } from '@material-ui/core';
import React, { Suspense } from 'react';
import { ThemeProvider } from '@material-ui/styles';
import { BrowserRouter, Switch, Route } from 'react-router-dom';
import theme from 'util/theme';
import HomePage from './HomePage';
import HardwareSpecDetailsView from './hardware/HardwareSpecDetailsView';
import ProgramSpecView from './programs/ProgramSpecView';
import NotFoundPage from './NotFoundPage';
import PageContainer from './common/PageContainer';
import environment from 'util/environment';
import { RelayEnvironmentProvider } from 'relay-hooks';
import DocsPage from 'components/docs/DocsPage';

const ProgramIdeView = React.lazy(() => import('./ide/ProgramIdeView'));

const App: React.FC = () => {
  return (
    <RelayEnvironmentProvider environment={environment}>
      <ThemeProvider theme={theme}>
        <CssBaseline />
        <BrowserRouter>
          <Suspense fallback={<CircularProgress />}>
            <Switch>
              {/* Full screen routes first */}
              <Route
                path="/hardware/:hwSlug/puzzles/:programSlug/:fileName"
                exact
              >
                <ProgramIdeView />
              </Route>

              {/* All non-full screen routes */}
              <Route path="*">
                <PageContainer>
                  <Switch>
                    <Route path="/" exact>
                      <HomePage />
                    </Route>

                    <Route path="/docs">
                      <DocsPage />
                    </Route>

                    {/* Hardware routes */}
                    <Route path="/hardware/:hwSlug" exact>
                      <HardwareSpecDetailsView />
                    </Route>
                    <Route path="/hardware/:hwSlug/puzzles/:programSlug" exact>
                      <ProgramSpecView />
                    </Route>

                    <Route path="*">
                      <NotFoundPage />
                    </Route>
                  </Switch>
                </PageContainer>
              </Route>
            </Switch>
          </Suspense>
        </BrowserRouter>
      </ThemeProvider>
    </RelayEnvironmentProvider>
  );
};

export default App;
